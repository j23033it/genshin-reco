import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import script from "../../src-tauri/src/windows-ime-focus.js?raw";

describe("Windowsの編成欄へのIME再接続", () => {
  let frame: HTMLIFrameElement;
  let doc: Document;
  let composer: HTMLTextAreaElement;
  let invoke: ReturnType<typeof vi.fn>;
  let install: () => void;

  beforeEach(() => {
    frame = document.createElement("iframe");
    document.body.append(frame);
    doc = frame.contentDocument!;
    composer = doc.createElement("textarea");
    composer.id = "team-request";
    doc.body.append(composer);
    invoke = vi.fn().mockResolvedValue(undefined);
    const bridge = { __TAURI_INTERNALS__: { invoke } };
    install = () => new Function("window", "document", "HTMLTextAreaElement", script)(
      bridge,
      doc,
      Object.getPrototypeOf(composer).constructor,
    );
    install();
    composer.focus();
  });

  afterEach(() => frame.remove());

  function click(target: Element = composer, button = 0) {
    target.dispatchEvent(new MouseEvent("pointerup", { button, bubbles: true }));
  }

  it("選ばれている編成欄をクリックした時に再接続する", () => {
    composer.value = "サンドローネ、";
    composer.setSelectionRange(3, 3);
    click();
    expect(invoke).toHaveBeenCalledWith("repair_windows_ime_focus", {
      requestedAt: expect.any(Number),
    });
    expect(composer.value).toBe("サンドローネ、");
    expect(composer.selectionStart).toBe(3);
  });

  it("右クリックや無効な欄からは再接続しない", () => {
    click(composer, 2);
    composer.disabled = true;
    click();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("別の欄や、選ばれていない編成欄から入力先を奪わない", () => {
    const other = doc.createElement("textarea");
    doc.body.append(other);
    other.focus();
    click(other);
    click();
    expect(invoke).not.toHaveBeenCalled();
    expect(doc.activeElement).toBe(other);
  });

  it("変換中のクリックでは動かず、変換終了後は再接続できる", () => {
    composer.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    click();
    expect(invoke).not.toHaveBeenCalled();
    composer.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true }));
    click();
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("待機中は重複せず、完了後のクリックは処理する", async () => {
    let complete!: () => void;
    invoke.mockReturnValue(new Promise<void>((resolve) => { complete = resolve; }));
    click();
    click();
    expect(invoke).toHaveBeenCalledTimes(1);
    complete();
    await Promise.resolve();
    await Promise.resolve();
    click();
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("ネイティブ処理に失敗しても次のクリックを処理できる", async () => {
    invoke.mockRejectedValueOnce(new Error("再接続できません"));
    click();
    await Promise.resolve();
    await Promise.resolve();
    click();
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("同じページへ二重に登録しない", () => {
    install();
    click();
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
