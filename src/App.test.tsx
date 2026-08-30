import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("App", () => {
  it("製品の位置づけとGate 0を表示する", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "根拠付き・編成連動ビルド推薦" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Gate 0");
  });
});
