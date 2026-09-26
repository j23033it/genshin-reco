import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ResearchConversation,
  ResearchMemberInput,
  ResearchProgress,
  ResearchRepository,
  ResearchStage,
  ResearchedTeamRecord,
  ResearchedTeamSummary,
} from "./types";

function errorText(error: unknown) {
  return error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "処理に失敗しました。もう一度お試しください。";
}

export function useResearchChat(repository: ResearchRepository) {
  const [stage, setStage] = useState<ResearchStage>("empty");
  const [conversation, setConversation] = useState<ResearchConversation | null>(
    null,
  );
  const [record, setRecord] = useState<ResearchedTeamRecord | null>(null);
  const [teams, setTeams] = useState<ResearchedTeamSummary[]>([]);
  const [listLoading, setListLoading] = useState(true);
  const [listError, setListError] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [activityLabel, setActivityLabel] = useState("読み込み中…");
  const [cancelling, setCancelling] = useState(false);
  const [progress, setProgress] = useState<ResearchProgress | null>(null);
  const operation = useRef({ id: 0 });
  const listOperation = useRef({ id: 0 });
  const locked = useRef(false);
  const cancelLocked = useRef(false);
  const succeededOperation = useRef<number | null>(null);
  const alive = useRef(true);
  const unsubscribe = useRef<(() => void) | null>(null);

  const refreshTeams = useCallback(() => {
    const request = ++listOperation.current.id;
    return Promise.resolve()
      .then(() => repository.listTeams())
      .then(
        (result) => {
          if (alive.current && request === listOperation.current.id) {
            setTeams(result);
            setListError("");
          }
        },
        (failure: unknown) => {
          if (alive.current && request === listOperation.current.id)
            setListError(errorText(failure));
        },
      )
      .finally(() => {
        if (alive.current && request === listOperation.current.id)
          setListLoading(false);
      });
  }, [repository]);

  useEffect(() => {
    const operationState = operation.current;
    const listState = listOperation.current;
    alive.current = true;
    void refreshTeams();
    return () => {
      alive.current = false;
      operationState.id++;
      listState.id++;
      unsubscribe.current?.();
      unsubscribe.current = null;
    };
  }, [refreshTeams]);

  function begin(label: string) {
    if (locked.current || cancelLocked.current) return null;
    locked.current = true;
    setBusy(true);
    setActivityLabel(label);
    setError("");
    return ++operation.current.id;
  }

  function current(request: number) {
    return alive.current && request === operation.current.id;
  }
  function finish(request: number) {
    if (!current(request)) return;
    locked.current = false;
    setBusy(false);
  }

  async function send(message: string) {
    const request = begin("編成の条件を確認中…");
    if (request === null) return false;
    try {
      const result = await repository.sendMessage(
        message,
        conversation?.sessionId,
      );
      if (!current(request)) return false;
      setConversation({
        ...result,
        title: result.title ?? (record?.sessionId === result.sessionId ? record.title : null),
      });
      setRecord(null);
      setStage(
        result.status === "ready"
          ? "ready"
          : result.status === "cancelled"
            ? "cancelled"
            : result.status === "failed"
              ? "error"
              : "collecting",
      );
      if (result.error) setError(result.error);
      return true;
    } catch (failure) {
      if (current(request)) {
        setError(errorText(failure));
        setStage("error");
      }
      return false;
    } finally {
      finish(request);
    }
  }

  async function start(members?: ResearchMemberInput[], title?: string | null) {
    if (
      !conversation ||
      !["ready", "failed", "cancelled"].includes(conversation.status)
    )
      return;
    const request = begin("調査の条件を準備中…");
    if (request === null) return;
    setProgress(null);
    let stop: (() => void) | undefined;
    try {
      const selectedConversation = members
        ? await repository.updateConditions(conversation.sessionId, members, title ?? null)
        : conversation;
      if (!current(request)) return;
      setConversation(selectedConversation);
      setStage("researching");
      const unlisten = await repository.subscribeProgress((event) => {
        if (current(request) && event.sessionId === selectedConversation.sessionId)
          setProgress(event);
      });
      let stopped = false;
      stop = () => {
        if (!stopped) {
          stopped = true;
          unlisten();
        }
      };
      if (!current(request)) return;
      unsubscribe.current = stop;
      const result = await repository.startResearch(selectedConversation.sessionId);
      if (!current(request)) return;
      succeededOperation.current = request;
      setRecord(result);
      setConversation((previous) =>
        previous?.sessionId === result.sessionId
          ? previous
          : {
              sessionId: result.sessionId,
              status: "succeeded",
              messages: [],
              members: result.members.map((member, slotIndex) => ({
                slotIndex,
                name: member.name,
              })),
              createdAt: result.createdAt,
              updatedAt: result.updatedAt,
            },
      );
      setError("");
      setStage("result");
      void refreshTeams();
    } catch (failure) {
      if (current(request)) {
        setError(errorText(failure));
        setStage("error");
      }
    } finally {
      stop?.();
      if (unsubscribe.current === stop) unsubscribe.current = null;
      finish(request);
    }
  }

  async function cancel() {
    if (!conversation || stage !== "researching" || cancelLocked.current)
      return;
    const request = operation.current.id;
    cancelLocked.current = true;
    setCancelling(true);
    try {
      await repository.cancelResearch(conversation.sessionId);
      if (!current(request) || succeededOperation.current === request) return;
      operation.current.id++;
      unsubscribe.current?.();
      unsubscribe.current = null;
      locked.current = false;
      setBusy(false);
      setStage("cancelled");
      setError("");
      void refreshTeams();
    } catch (failure) {
      if (current(request) && locked.current)
        setError(`キャンセルできませんでした。${errorText(failure)}`);
    } finally {
      cancelLocked.current = false;
      if (alive.current) setCancelling(false);
    }
  }

  async function openTeam(teamId: string) {
    const request = begin("保存した編成を読み込み中…");
    if (request === null) return;
    try {
      const result = await repository.loadTeam(teamId);
      if (!current(request)) return;
      if (!result)
        throw new Error(
          "この編成は見つかりません。保存一覧を更新してください。",
        );
      const restored = await repository.loadConversation(result.sessionId);
      if (!current(request)) return;
      setRecord(result);
      setConversation(
        (previous) =>
          (restored ? { ...restored, title: restored.title ?? result.title } : null) ??
          (previous?.sessionId === result.sessionId
            ? previous
            : {
                sessionId: result.sessionId,
                status: "succeeded",
                messages: [],
                members: result.members.map((member, slotIndex) => ({
                  slotIndex,
                  name: member.name,
                })),
                title: result.title,
                createdAt: result.createdAt,
                updatedAt: result.updatedAt,
              }),
      );
      setError("");
      setStage("result");
    } catch (failure) {
      if (current(request)) {
        setError(errorText(failure));
        setStage("error");
      }
    } finally {
      finish(request);
    }
  }

  async function renameTeam(teamId: string, title: string) {
    const request = begin("編成名を保存中…");
    if (request === null) throw new Error("別の処理が完了してから変更してください。");
    try {
      const result = await repository.renameTeam(teamId, title);
      if (!current(request)) return;
      setRecord((previous) => previous?.teamId === teamId ? result : previous);
      setConversation((previous) => previous?.sessionId === result.sessionId
        ? { ...previous, title: result.title }
        : previous);
      setTeams((previous) => previous.map((team) => team.teamId === teamId
        ? { ...team, title: result.title, updatedAt: result.updatedAt }
        : team));
      void refreshTeams();
    } finally {
      finish(request);
    }
  }

  function reset() {
    if (locked.current) return;
    operation.current.id++;
    setConversation(null);
    setRecord(null);
    setError("");
    setProgress(null);
    setStage("empty");
  }

  return {
    stage,
    conversation,
    record,
    teams,
    busy: busy || cancelling,
    activityLabel: cancelling ? "調査をキャンセル中…" : activityLabel,
    cancelling,
    error,
    progress,
    listLoading,
    listError,
    send,
    start,
    cancel,
    openTeam,
    renameTeam,
    reset,
    refreshTeams: () => {
      setListLoading(true);
      setListError("");
      return refreshTeams();
    },
  };
}
