import "@testing-library/jest-dom/vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ResearchChatPrototype } from "./ResearchChatPrototype";
import type {
  ResearchConversation,
  ResearchProgress,
  ResearchRepository,
  ResearchedTeamRecord,
} from "./types";

vi.mock("../catalog/loadCatalog", () => ({
  loadCatalog: vi.fn(async () => ({
    schemaVersion: "catalog-v2",
    gameVersion: "7.0",
    catalogUpdatedAt: "2026-08-24",
    characters: [
      { id: "a", name: "アルレッキーノ", element: "炎", weaponType: "長柄武器", rarity: 5, imageUrl: "" },
      { id: "b", name: "夜蘭", element: "水", weaponType: "弓", rarity: 5, imageUrl: "" },
    ],
    weapons: [
      { id: "spear", name: "赤月のシルエット", weaponType: "長柄武器", rarity: 5, imageUrl: "" },
      { id: "spear-2", name: "和璞鳶", weaponType: "長柄武器", rarity: 5, imageUrl: "" },
      { id: "bow", name: "若水", weaponType: "弓", rarity: 5, imageUrl: "" },
    ],
    artifactSets: [],
  })),
}));

const conversation: ResearchConversation = {
  sessionId: "session-1",
  status: "ready",
  messages: [
    { role: "assistant", content: "条件が揃いました", createdAt: "2026-09-25" },
  ],
  members: [{ slotIndex: 0, name: "テストキャラ" }],
  createdAt: "2026-09-25",
  updatedAt: "2026-09-25",
};
const record: ResearchedTeamRecord = {
  teamId: "team-1",
  sessionId: "session-1",
  title: "テスト編成",
  sources: [],
  warnings: [],
  createdAt: "2026-09-25",
  updatedAt: "2026-09-25",
  members: [
    {
      id: "a",
      name: "テストキャラ",
      element: "岩",
      role: "シールド",
      constellation: "無凸",
      weapon: "長い武器の名前".repeat(10),
      artifact: "聖遺物",
      mainStats: "HP% / HP% / HP%",
      subStats: "HP",
      targetStats: [
        { label: "HP", value: "45,000", primary: true, note: "固有天賦の発動条件を確認し、編成効果を含めず戦闘前に合わせる。" },
        { label: "元素熟知" },
      ],
    },
  ],
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function setupRepository() {
  let progress: (event: ResearchProgress) => void = () => {};
  const stop = vi.fn();
  const repository: ResearchRepository = {
    mode: "tauri",
    sendMessage: vi.fn(async () => conversation),
    updateConditions: vi.fn(async (_sessionId, members, title) => ({
      ...conversation,
      members,
      title,
    })),
    startResearch: vi.fn(async () => record),
    cancelResearch: vi.fn(async () => {}),
    listTeams: vi.fn(async () => []),
    loadConversation: vi.fn(async () => null),
    loadTeam: vi.fn(async () => record),
    renameTeam: vi.fn(async (_teamId, title) => ({ ...record, title })),
    subscribeProgress: vi.fn(async (callback) => {
      progress = callback;
      return stop;
    }),
  };
  return {
    repository,
    stop,
    emit: (event: ResearchProgress) => progress(event),
  };
}
async function send(
  user: ReturnType<typeof userEvent.setup>,
  message = "4人を調べたい",
) {
  await user.type(
    screen.getByRole("textbox", { name: "調べたい編成" }),
    message,
  );
  await user.click(screen.getByRole("button", { name: "送信" }));
}

describe("実データの編成調査UI", () => {
  it("初回表示と新規ボタン操作後はクリックせず入力できる", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    render(<ResearchChatPrototype repository={repository} />);
    const input = screen.getByRole("textbox", { name: "調べたい編成" });
    expect(input).toHaveFocus();
    await user.keyboard("サンドローネ、");
    expect(input).toHaveValue("サンドローネ、");

    await user.click(screen.getByRole("button", { name: "新しい編成を調べる" }));
    expect(input).toHaveValue("");
    expect(input).toHaveFocus();
    await user.keyboard("あ");
    expect(input).toHaveValue("あ");
    expect(repository.sendMessage).not.toHaveBeenCalled();
  });

  it("初回表示でも既に選ばれた別の入力からフォーカスを奪わない", () => {
    const { repository } = setupRepository();
    render(<input aria-label="別の入力" />);
    const other = screen.getByRole("textbox", { name: "別の入力" });
    other.focus();
    render(<ResearchChatPrototype repository={repository} />);
    expect(other).toHaveFocus();
  });

  it("狭い画面の新規操作はサイドバーを閉じて入力欄へ移る", async () => {
    vi.stubGlobal("matchMedia", vi.fn((query: string) => ({
      matches: query.includes("max-width"),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })));
    try {
      const user = userEvent.setup();
      const { repository } = setupRepository();
      const view = render(<ResearchChatPrototype repository={repository} />);
      await user.click(screen.getByRole("button", { name: "サイドバーを開く" }));
      await user.click(screen.getByRole("button", { name: "新しい編成を調べる" }));
      expect(screen.queryByRole("complementary")).not.toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "調べたい編成" })).toHaveFocus();
      view.unmount();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it.each(["成功", "失敗"])("送信%s後は無効状態が解除されてから入力欄へ戻る", async (outcome) => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    const request = deferred<ResearchConversation>();
    vi.mocked(repository.sendMessage).mockReturnValueOnce(request.promise);
    render(<ResearchChatPrototype repository={repository} />);
    await send(user, "入力を保持");
    const input = screen.getByRole("textbox", { name: "調べたい編成" });
    expect(screen.getByRole("progressbar", { name: "編成の条件を確認中…" })).not.toHaveAttribute("aria-valuenow");
    expect(input).toBeDisabled();
    // 実ブラウザでは無効になった送信ボタンのフォーカスが外れる。
    (document.activeElement as HTMLElement).blur();
    await act(async () => {
      if (outcome === "成功") request.resolve(conversation);
      else request.reject(new Error("接続に失敗"));
    });
    expect(input).toBeEnabled();
    expect(input).toHaveFocus();
    expect(input).toHaveValue(outcome === "成功" ? "" : "入力を保持");
  });

  it("送信待ちに別の入力を選んだ場合は完了後もそこに留まる", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    const request = deferred<ResearchConversation>();
    vi.mocked(repository.sendMessage).mockReturnValueOnce(request.promise);
    render(<><input aria-label="別の入力" /><ResearchChatPrototype repository={repository} /></>);
    await send(user);
    const other = screen.getByRole("textbox", { name: "別の入力" });
    await user.click(other);
    await act(async () => request.resolve(conversation));
    expect(other).toHaveFocus();
  });

  it("Enterで送信し、成功したら入力欄を空にする", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    render(<ResearchChatPrototype repository={repository} />);
    const input = screen.getByRole("textbox", { name: "調べたい編成" });

    await user.type(input, "  4人を調べたい  {Enter}");

    expect(repository.sendMessage).toHaveBeenCalledExactlyOnceWith(
      "4人を調べたい",
      undefined,
      "genshin",
    );
    await waitFor(() => expect(input).toHaveValue(""));
  });

  it("Shift+Enterで改行し、Enterで複数行をまとめて送信する", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    render(<ResearchChatPrototype repository={repository} />);
    const input = screen.getByRole("textbox", { name: "調べたい編成" });

    await user.type(input, "4人を調べたい{Shift>}{Enter}{/Shift}全員無凸です");

    expect(input).toHaveValue("4人を調べたい\n全員無凸です");
    expect(repository.sendMessage).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    expect(repository.sendMessage).toHaveBeenCalledExactlyOnceWith(
      "4人を調べたい\n全員無凸です",
      undefined,
      "genshin",
    );
  });

  it.each([
    { isComposing: true },
    { keyCode: 229 },
  ])("日本語変換中のEnterでは送信しない（%j）", async (composition) => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    render(<ResearchChatPrototype repository={repository} />);
    const input = screen.getByRole("textbox", { name: "調べたい編成" });
    await user.type(input, "全員無凸です");

    expect(fireEvent.keyDown(input, { key: "Enter", ...composition })).toBe(true);
    expect(repository.sendMessage).not.toHaveBeenCalled();
    expect(input).toHaveValue("全員無凸です");
    await user.keyboard("{Enter}");
    expect(repository.sendMessage).toHaveBeenCalledExactlyOnceWith(
      "全員無凸です",
      undefined,
      "genshin",
    );
  });

  it("空欄や空白だけの入力、Enterの長押しでは送信しない", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    render(<ResearchChatPrototype repository={repository} />);
    const input = screen.getByRole("textbox", { name: "調べたい編成" });

    await user.type(input, "{Enter}   {Enter}");
    expect(input).toHaveValue("   ");
    expect(repository.sendMessage).not.toHaveBeenCalled();
    await user.clear(input);
    await user.type(input, "4人を調べたい");
    fireEvent.keyDown(input, { key: "Enter", repeat: true });
    expect(repository.sendMessage).not.toHaveBeenCalled();
    expect(input).toHaveValue("4人を調べたい");
  });

  it("4人が揃うと凸と武器を選べ、指定なしを保ったまま保存してから調査する", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    vi.mocked(repository.sendMessage).mockResolvedValueOnce({
      ...conversation,
      members: ["アルレッキーノ", "夜蘭", "ベネット", "鍾離"].map((name, slotIndex) => ({
        slotIndex,
        name,
        weapon: slotIndex === 1 ? "若水" : null,
      })),
    });
    render(<ResearchChatPrototype repository={repository} />);
    await send(user, "アルレッキーノ、夜蘭、ベネット、鍾離");
    await user.type(await screen.findByRole("textbox", { name: "編成名" }), "蒸発編成");
    const attacker = await screen.findByRole("group", { name: "アルレッキーノの条件" });
    expect(within(attacker).getByRole("button", { name: "アルレッキーノ：指定なし" })).toHaveAttribute("aria-pressed", "true");
    await user.click(within(attacker).getByRole("button", { name: "アルレッキーノ：2凸" }));
    const weapon = within(attacker).getByRole("combobox", { name: "武器" });
    expect(within(weapon).queryByRole("option", { name: "若水" })).not.toBeInTheDocument();
    await user.selectOptions(weapon, "赤月のシルエット");
    await user.selectOptions(weapon, "");
    await user.selectOptions(weapon, "赤月のシルエット");
    const hydro = screen.getByRole("group", { name: "夜蘭の条件" });
    await user.selectOptions(within(hydro).getByRole("combobox", { name: "武器" }), "");
    await user.click(screen.getByRole("button", { name: "この条件で調査する" }));
    await waitFor(() => expect(repository.updateConditions).toHaveBeenCalledOnce());
    const [sessionId, selected, title] = vi.mocked(repository.updateConditions).mock.calls[0];
    expect(sessionId).toBe("session-1");
    expect(title).toBe("蒸発編成");
    expect(selected[0]).toMatchObject({ constellation: 2, weapon: "赤月のシルエット", refinement: null });
    expect(selected[1]).toMatchObject({ weapon: null, refinement: null });
    await waitFor(() => expect(repository.startResearch).toHaveBeenCalledWith("session-1"));
  }, 15000);

  it("不足情報を確認し、同じ会話へ追加して準備完了になったときだけ開始できる", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    vi.mocked(repository.sendMessage).mockResolvedValueOnce({
      ...conversation,
      status: "collecting",
      messages: [
        {
          role: "assistant",
          content: "武器と凸数を教えてください",
          createdAt: "now",
        },
      ],
    });
    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    expect(
      await screen.findByText("武器と凸数を教えてください"),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /この内容で調査する/ }),
    ).not.toBeInTheDocument();
    await send(user, "全員無凸です");
    expect(repository.sendMessage).toHaveBeenLastCalledWith(
      "全員無凸です",
      "session-1",
      "genshin",
    );
    expect(
      await screen.findByRole("button", { name: /この内容で調査する/ }),
    ).toBeEnabled();
  });

  it("二重送信を防ぎ、失敗しても入力を保って再送信できる", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    const request = deferred<ResearchConversation>();
    vi.mocked(repository.sendMessage).mockReturnValueOnce(request.promise);
    render(<ResearchChatPrototype repository={repository} />);
    await user.type(screen.getByRole("textbox"), "入力を保持{Enter}");
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Enter" });
    fireEvent.submit(screen.getByRole("textbox").closest("form")!);
    expect(repository.sendMessage).toHaveBeenCalledTimes(1);
    await act(async () => request.reject(new Error("接続に失敗")));
    expect(screen.getByRole("textbox")).toHaveValue("入力を保持");
    expect(screen.getByRole("alert")).toHaveTextContent("接続に失敗");
    await user.keyboard("{Enter}");
    expect(
      await screen.findByRole("button", { name: /この内容で調査する/ }),
    ).toBeEnabled();
  });

  it("進捗のsessionIdを照合し、完了結果と主参照値を表示して購読を解除する", async () => {
    const user = userEvent.setup();
    const { repository, emit, stop } = setupRepository();
    const request = deferred<ResearchedTeamRecord>();
    vi.mocked(repository.startResearch).mockReturnValue(request.promise);
    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    await user.click(
      screen.getByRole("button", { name: /この内容で調査する/ }),
    );
    expect(repository.subscribeProgress).toHaveBeenCalledTimes(1);
    const steps = screen.getByRole("list", { name: "調査の段階" });
    expect(within(steps).getByText("準備").closest("li")).toHaveAttribute("aria-current", "step");
    expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    act(() => emit({ sessionId: "old", stage: "weapons", detail: "古い進捗" }));
    expect(screen.queryByText("古い進捗")).not.toBeInTheDocument();
    act(() =>
      emit({
        sessionId: "session-1",
        stage: "weapons",
        detail: "武器を確認中",
        memberName: "テストキャラ",
      }),
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "テストキャラ：武器を確認中",
    );
    expect(within(steps).getByText("調査").closest("li")).toHaveAttribute("aria-current", "step");
    act(() => emit({ sessionId: "session-1", stage: "validating", detail: "結果を確認中" }));
    expect(within(steps).getByText("検証・保存").closest("li")).toHaveAttribute("aria-current", "step");
    expect(screen.getByRole("status")).toHaveTextContent("結果を確認中");
    expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    await act(async () => request.resolve(record));
    expect(
      screen.getByRole("heading", { name: "テスト編成" }),
    ).toBeInTheDocument();
    expect(screen.getByText("主参照")).toBeInTheDocument();
    expect(screen.getByText("45,000")).toBeInTheDocument();
    expect(screen.getByText("固有天賦の発動条件を確認し、編成効果を含めず戦闘前に合わせる。")).toBeInTheDocument();
    expect(screen.getByText("数値は未確認")).toBeInTheDocument();
    expect(screen.getByText("テストキャラ（画像なし）")).toBeInTheDocument();
    expect(stop).toHaveBeenCalled();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(repository.listTeams).toHaveBeenCalledTimes(2);
  });

  it("再調査では4人を固定し、武器と凸を選び直して文字入力なしで開始する", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    const names = ["アルレッキーノ", "夜蘭", "ベネット", "鍾離"];
    const members = names.map((name, slotIndex) => ({
      slotIndex,
      name,
      weapon: slotIndex === 0 ? "赤月のシルエット" : null,
      constellation: slotIndex === 0 ? 1 : null,
      refinement: null,
    }));
    vi.mocked(repository.sendMessage).mockResolvedValueOnce({ ...conversation, members });
    vi.mocked(repository.startResearch).mockResolvedValue({
      ...record,
      members: names.map((name, index) => ({ ...record.members[0], id: `${index}`, name })),
    });
    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    await user.click(await screen.findByRole("button", { name: "この条件で調査する" }));
    await screen.findByRole("heading", { name: "テスト編成" });

    await user.click(screen.getByRole("button", { name: "条件を変えて再調査" }));
    expect(screen.getByRole("button", { name: "結果に戻る" })).toHaveFocus();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    const attacker = screen.getByRole("group", { name: "アルレッキーノの条件" });
    expect(within(attacker).getByRole("button", { name: "アルレッキーノ：1凸" })).toHaveAttribute("aria-pressed", "true");
    expect(within(attacker).getByRole("combobox", { name: "武器" })).toHaveValue("赤月のシルエット");
    await user.click(within(attacker).getByRole("button", { name: "アルレッキーノ：2凸" }));
    await user.selectOptions(within(attacker).getByRole("combobox", { name: "武器" }), "和璞鳶");
    await user.click(screen.getByRole("button", { name: "この条件で再調査する" }));
    await waitFor(() => expect(repository.updateConditions).toHaveBeenCalledTimes(2));
    expect(vi.mocked(repository.updateConditions).mock.calls[1][1][0]).toMatchObject({
      name: "アルレッキーノ", constellation: 2, weapon: "和璞鳶",
    });
    expect(repository.sendMessage).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(repository.startResearch).toHaveBeenCalledTimes(2));
  });

  it("キャンセル後の遅い完了を無視し、再調査できる", async () => {
    const user = userEvent.setup();
    const { repository, stop } = setupRepository();
    const request = deferred<ResearchedTeamRecord>();
    vi.mocked(repository.startResearch).mockReturnValueOnce(request.promise);
    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    await user.click(
      screen.getByRole("button", { name: /この内容で調査する/ }),
    );
    await user.click(screen.getByRole("button", { name: "調査をキャンセル" }));
    expect(repository.cancelResearch).toHaveBeenCalledWith("session-1");
    expect(
      await screen.findByText(/調査をキャンセルしました/),
    ).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    await act(async () => request.resolve(record));
    expect(
      screen.queryByRole("heading", { name: "テスト編成" }),
    ).not.toBeInTheDocument();
    expect(stop).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "もう一度調査する" }));
    expect(
      await screen.findByRole("heading", { name: "テスト編成" }),
    ).toBeInTheDocument();
  });

  it("調査失敗から再試行でき、購読を解除する", async () => {
    const user = userEvent.setup();
    const { repository, stop } = setupRepository();
    vi.mocked(repository.startResearch).mockRejectedValueOnce(
      new Error("調査に失敗"),
    );
    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    await user.click(
      screen.getByRole("button", { name: /この内容で調査する/ }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("調査に失敗");
    expect(stop).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "もう一度調査する" }));
    expect(
      await screen.findByRole("heading", { name: "テスト編成" }),
    ).toBeInTheDocument();
  });

  it("保存一覧の失敗を再取得し、保存結果を読み戻して条件を変更できる", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    vi.mocked(repository.listTeams)
      .mockRejectedValueOnce(new Error("一覧取得失敗"))
      .mockResolvedValue([
        {
          teamId: "team-1",
          title: "テスト編成",
          memberNames: ["テストキャラ"],
          memberImageUrls: [],
          updatedAt: "now",
        },
      ]);
    vi.mocked(repository.loadTeam).mockResolvedValue({
      ...record,
      members: [
        {
          ...record.members[0],
          targetStats: null,
          imageUrl: "https://example.com/missing.png",
        },
        ...["夜蘭", "ベネット", "鍾離"].map((name, index) => ({
          ...record.members[0], id: `${index + 1}`, name,
        })),
      ],
    });
    render(<ResearchChatPrototype repository={repository} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("一覧取得失敗");
    await user.click(screen.getByRole("button", { name: "保存一覧を更新" }));
    await user.click(await screen.findByRole("button", { name: /テスト編成/ }));
    expect(
      await screen.findByRole("heading", { name: "テスト編成" }),
    ).toBeInTheDocument();
    expect(repository.loadTeam).toHaveBeenCalledWith("team-1");
    expect(screen.getByText("目標値は未確認です。")).toBeInTheDocument();
    fireEvent.error(screen.getByRole("img", { name: "テストキャラ" }));
    expect(screen.getByText("テストキャラ（画像なし）")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "条件を変えて再調査" }),
    );
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.getByRole("group", { name: "テストキャラの条件" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "夜蘭の条件" })).toBeInTheDocument();
    // This legacy fixture contains unknown characters/equipment. Confirm before resubmitting.
    expect(screen.getByRole("button", { name: "この条件で再調査する" })).toBeDisabled();
    for (const select of screen.getAllByRole("combobox", { name: "武器" })) {
      await waitFor(() => expect(select).toBeEnabled());
      await user.selectOptions(select, "");
    }
    await user.click(screen.getByRole("button", { name: "この条件で再調査する" }));
    await waitFor(() => expect(repository.updateConditions).toHaveBeenCalledWith(
      "session-1", expect.arrayContaining([expect.objectContaining({ name: "テストキャラ" })]), "テスト編成",
    ));
    expect(repository.sendMessage).not.toHaveBeenCalled();
  });

  it("保存編成と一緒に過去の会話を復元する", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    vi.mocked(repository.listTeams).mockResolvedValue([
      {
        teamId: "team-1",
        title: "テスト編成",
        memberNames: [],
        memberImageUrls: [],
        updatedAt: "now",
      },
    ]);
    vi.mocked(repository.loadConversation).mockResolvedValue({
      ...conversation,
      status: "succeeded",
      messages: [
        { role: "user", content: "保存されていた指定条件", createdAt: "now" },
      ],
    });
    render(<ResearchChatPrototype repository={repository} />);
    await user.click(await screen.findByRole("button", { name: "テスト編成" }));
    await user.click(await screen.findByRole("button", { name: "会話" }));
    expect(screen.getByText("保存されていた指定条件")).toBeInTheDocument();
    expect(repository.loadConversation).toHaveBeenCalledWith("session-1");
  });

  it("注意点のない保存結果には再調査の案内を表示する", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    vi.mocked(repository.startResearch).mockResolvedValue({
      ...record,
      members: [{
        ...record.members[0],
        targetStats: [{ label: "会心率", value: "70%", note: null }],
      }],
    });

    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    await user.click(screen.getByRole("button", { name: /この内容で調査する/ }));

    expect(await screen.findByText(/目標値の個別の注意点がありません/)).toBeInTheDocument();
  });

  it("保存済みの編成名を変更し、結果と一覧へ反映する", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    let title = "テスト編成";
    vi.mocked(repository.listTeams).mockImplementation(async () => [{
      teamId: "team-1",
      title,
      memberNames: ["テストキャラ"],
      memberImageUrls: [],
      updatedAt: "now",
    }]);
    vi.mocked(repository.renameTeam).mockImplementation(async (_teamId, nextTitle) => {
      title = nextTitle;
      return { ...record, title };
    });
    render(<ResearchChatPrototype repository={repository} />);
    await user.click(await screen.findByRole("button", { name: /テスト編成/ }));
    await user.click(await screen.findByRole("button", { name: "編成名を編集" }));
    const input = screen.getByRole("textbox", { name: "保存済みの編成名" });
    await user.clear(input);
    await user.type(input, "新しい編成名");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(repository.renameTeam).toHaveBeenCalledWith("team-1", "新しい編成名");
    expect(await screen.findByRole("heading", { name: "新しい編成名" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: /新しい編成名/ })).toBeInTheDocument();
  });

  it("キャンセル失敗を表示して調査の完了を待てる", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    const request = deferred<ResearchedTeamRecord>();
    vi.mocked(repository.startResearch).mockReturnValueOnce(request.promise);
    vi.mocked(repository.cancelResearch).mockRejectedValueOnce(
      new Error("接続失敗"),
    );
    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    await user.click(
      screen.getByRole("button", { name: /この内容で調査する/ }),
    );
    await user.click(screen.getByRole("button", { name: "調査をキャンセル" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "キャンセルできませんでした",
    );
    expect(
      screen.getByRole("button", { name: "調査をキャンセル" }),
    ).toBeEnabled();
    await act(async () => request.resolve(record));
    expect(
      screen.getByRole("heading", { name: "テスト編成" }),
    ).toBeInTheDocument();
  });

  it("キャンセル応答より先に調査が中断エラーになってもキャンセル状態を保つ", async () => {
    const user = userEvent.setup();
    const { repository } = setupRepository();
    const research = deferred<ResearchedTeamRecord>();
    const cancellation = deferred<void>();
    vi.mocked(repository.startResearch).mockReturnValueOnce(research.promise);
    vi.mocked(repository.cancelResearch).mockReturnValueOnce(
      cancellation.promise,
    );
    render(<ResearchChatPrototype repository={repository} />);
    await send(user);
    await user.click(
      screen.getByRole("button", { name: /この内容で調査する/ }),
    );
    await user.click(screen.getByRole("button", { name: "調査をキャンセル" }));
    await act(async () => research.reject(new Error("調査が中断されました")));
    await act(async () => cancellation.resolve());
    expect(screen.getByText(/調査をキャンセルしました/)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "もう一度調査する" }),
    ).toBeEnabled();
  });

  it("購読処理の途中で画面を離れても解除し、調査を開始しない", async () => {
    const user = userEvent.setup();
    const { repository, stop } = setupRepository();
    const subscription = deferred<() => void>();
    vi.mocked(repository.subscribeProgress).mockReturnValue(
      subscription.promise,
    );
    const { unmount } = render(
      <ResearchChatPrototype repository={repository} />,
    );
    await send(user);
    await user.click(
      screen.getByRole("button", { name: /この内容で調査する/ }),
    );
    unmount();
    await act(async () => subscription.resolve(stop));
    await waitFor(() => expect(stop).toHaveBeenCalled());
    expect(repository.startResearch).not.toHaveBeenCalled();
  });
});
