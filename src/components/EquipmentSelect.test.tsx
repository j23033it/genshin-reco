import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import { EquipmentSelect } from "./EquipmentSelect";

afterEach(cleanup);
const options = [
  { value: "moon", label: "赤月のシルエット" },
  { value: "jade", label: "和璞鳶" },
  { value: "test", label: "テスト Blade" },
];

function ControlledSelect({ onChange = vi.fn() }: { onChange?: (value: string) => void }) {
  const [value, setValue] = useState("moon");
  return <EquipmentSelect id="weapon" label="武器" value={value} options={options} onChange={value => { setValue(value); onChange(value); }} />;
}

describe("装備検索と既存の選択", () => {
  it("名前の一部で絞り込み、選択と検索を独立して保持する", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<ControlledSelect onChange={onChange} />);
    const search = screen.getByRole("searchbox", { name: "武器を検索" });
    const select = screen.getByRole("combobox", { name: "武器" });
    expect(within(select).getAllByRole("option")).toHaveLength(4);
    await user.type(search, "和璞");
    expect(select).toHaveValue("moon");
    expect(within(select).queryByRole("option", { name: "テスト Blade" })).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("1件が一致しました。 選択中の装備は保持しています。");
    expect(onChange).not.toHaveBeenCalled();
    await user.selectOptions(select, "jade");
    expect(onChange).toHaveBeenLastCalledWith("jade");
    expect(within(select).queryByRole("option", { name: /赤月/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "武器の検索をクリア" }));
    expect(search).toHaveValue("");
    expect(search).toHaveFocus();
    expect(select).toHaveValue("jade");
    expect(within(select).getAllByRole("option")).toHaveLength(4);
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("該当なしでも選択を保持し、指定なしへの変更とEscでの検索解除ができる", async () => {
    const user = userEvent.setup();
    render(<ControlledSelect />);
    const search = screen.getByRole("searchbox");
    const select = screen.getByRole("combobox");
    await user.type(search, "ない名前");
    expect(screen.getByRole("status")).toHaveTextContent("一致する武器はありません。");
    expect(select).toHaveValue("moon");
    await user.selectOptions(select, "");
    expect(select).toHaveValue("");
    expect(within(select).getAllByRole("option")).toHaveLength(1);
    await user.click(search);
    await user.keyboard("{Escape}");
    expect(search).toHaveValue("");
    expect(within(select).getAllByRole("option")).toHaveLength(4);
    await user.type(search, "　ｂＬＡＤＥ　");
    expect(screen.getByRole("status")).toHaveTextContent("1件が一致しました。");
    expect(within(select).getByRole("option", { name: "テスト Blade" })).toBeInTheDocument();
  });

  it("日本語変換中のEnterとEscを奪わず、変換確定後に候補を絞る", () => {
    const onChange = vi.fn();
    render(<ControlledSelect onChange={onChange} />);
    const search = screen.getByRole("searchbox");
    const select = screen.getByRole("combobox");
    fireEvent.compositionStart(search);
    fireEvent.change(search, { target: { value: "わはく" } });
    expect(within(select).getAllByRole("option")).toHaveLength(4);
    expect(fireEvent.keyDown(search, { key: "Escape", isComposing: true })).toBe(true);
    expect(search).toHaveValue("わはく");
    expect(fireEvent.keyDown(search, { key: "Enter", isComposing: true })).toBe(true);
    fireEvent.change(search, { target: { value: "和璞鳶" } });
    fireEvent.compositionEnd(search, { data: "和璞鳶" });
    expect(screen.getByRole("status")).toHaveTextContent("1件が一致しました。");
    expect(select).toHaveValue("moon");
    expect(onChange).not.toHaveBeenCalled();
    expect(fireEvent.keyDown(search, { key: "Escape", keyCode: 229 })).toBe(true);
    expect(search).toHaveValue("和璞鳶");
  });

  it("Tabで検索から一覧へ移動でき、クリア後は検索へ戻る", async () => {
    const user = userEvent.setup();
    render(<ControlledSelect />);
    const search = screen.getByRole("searchbox");
    search.focus();
    await user.tab();
    expect(screen.getByRole("combobox")).toHaveFocus();
    await user.tab({ shift: true });
    await user.type(search, "赤月");
    await user.tab();
    expect(screen.getByRole("button", { name: "武器の検索をクリア" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(search).toHaveFocus();
    expect(search).toHaveValue("");
  });

  it("処理中は検索・クリア・選択を無効にする", async () => {
    const user = userEvent.setup();
    const props = { id: "weapon", label: "武器", value: "moon", options, onChange: vi.fn() };
    const { rerender } = render(<EquipmentSelect {...props} />);
    await user.type(screen.getByRole("searchbox"), "赤月");
    rerender(<EquipmentSelect {...props} disabled />);
    expect(screen.getByRole("searchbox")).toBeDisabled();
    expect(screen.getByRole("button")).toBeDisabled();
    expect(screen.getByRole("combobox")).toBeDisabled();
    expect(props.onChange).not.toHaveBeenCalled();
  });
});
