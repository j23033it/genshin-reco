import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ResearchChatPrototype } from "./ResearchChatPrototype";

describe("編成調査チャットのUI", () => {
  it("空の状態から4人を送り、不足情報の確認へ進める", async () => {
    const user = userEvent.setup();
    render(<ResearchChatPrototype />);

    expect(screen.getByRole("heading", { name: "調べたい4人を教えてください" })).toBeInTheDocument();
    expect(screen.getByText(/調査が完了した編成が/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /アルレッキーノ、夜蘭/ }));
    expect(screen.getByRole("textbox", { name: "調べたい編成" })).toHaveValue(
      "アルレッキーノ、夜蘭、ベネット、鍾離の4人を調べたい。",
    );
    await user.click(screen.getByRole("button", { name: "送信" }));

    expect(screen.getByText("4人を確認しました")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /この内容で調査する/ })).toBeEnabled();
  });

  it("調査完了後に画像付き結果と保存編成を表示する", async () => {
    const user = userEvent.setup();
    render(<ResearchChatPrototype />);

    await user.type(screen.getByRole("textbox", { name: "調べたい編成" }), "指定した4人を調べたい");
    await user.click(screen.getByRole("button", { name: "送信" }));
    await user.click(screen.getByRole("button", { name: /この内容で調査する/ }));

    expect(screen.getByRole("status")).toHaveTextContent("Codexが調査中");

    expect(await screen.findByRole("heading", { name: "アルレッキーノ蒸発編成" }, { timeout: 2500 })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "ビルド" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getAllByText("おすすめ武器")).toHaveLength(4);
    expect(screen.getAllByText("目標ステータス")).toHaveLength(4);
    expect(screen.getAllByText("主参照")).toHaveLength(4);
    expect(screen.getByText("2,000–2,300")).toBeInTheDocument();
    expect(screen.getByText("32,000–36,000")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /アルレッキーノ蒸発編成/ })).toHaveAttribute("aria-current", "page");
  });
});
