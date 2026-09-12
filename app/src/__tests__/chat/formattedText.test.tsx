import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { FormattedText } from "../../components/FormattedText";
import { structureReply } from "../../lib/structureReply";

describe("FormattedText", () => {
  it("renders bullet lists", () => {
    const { container } = render(
      <FormattedText text={"- one\n- two\n- three"} />,
    );
    const items = container.querySelectorAll("ul li");
    expect(items).toHaveLength(3);
    expect(items[0].textContent).toBe("one");
    expect(items[2].textContent).toBe("three");
  });

  it("renders numbered lists", () => {
    const { container } = render(
      <FormattedText text={"1. first\n2. second"} />,
    );
    const items = container.querySelectorAll("ol li");
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toBe("first");
    expect(items[1].textContent).toBe("second");
  });

  it("renders headings", () => {
    render(<FormattedText text={"## Next steps\nDo the thing"} />);
    expect(screen.getByRole("heading", { name: "Next steps" })).toBeTruthy();
  });

  it("renders inline bold, italic, and code", () => {
    const { container } = render(
      <FormattedText text={"**bold** and *italic* plus `code`"} />,
    );
    expect(container.querySelector("strong")?.textContent).toBe("bold");
    expect(container.querySelector("em")?.textContent).toBe("italic");
    expect(container.querySelector("code")?.textContent).toBe("code");
  });

  it("renders fenced code blocks", () => {
    const { container } = render(
      <FormattedText text={"```ts\nconst x = 1;\n```"} />,
    );
    expect(container.querySelector("pre code")?.textContent).toBe(
      "const x = 1;",
    );
  });

  it("renders links", () => {
    render(<FormattedText text={"See [docs](https://example.com)"} />);
    const link = screen.getByRole("link", { name: "docs" });
    expect(link.getAttribute("href")).toBe("https://example.com");
  });

  it("breaks a dense calendar reply into labels and lists", () => {
    const dense =
      "Work is on Mon 10 Aug, 8:45 AM–4:45 PM. Sleep 10:30 PM–7:45 AM. Best free times: on Mon 10 Aug, 5:55 PM–6:55 PM, on Mon 10 Aug, 6:40 PM–7:40 PM, and on Mon 10 Aug, 7:25 PM–8:25 PM. Study is on Mon 10 Aug, 4:45 PM–5:45 PM. Found 1 spark: “to invest in robotics company like hywai and boston dynamics, i want to study ro”. Found 5 tasks: “Easy task 2”, “Easy task 1”, “groom”, “clean kitchen”, and “fold laundry”. Proposed 3 blocks: “to invest in robotics company like hywai and boston dynamic...” on Mon 10 Aug, 5:55 PM–6:40 PM, “Easy task 2” on Mon 10 Aug, 6:50 PM–7:20 PM, and “Easy task 1” on Mon 10 Aug, 7:30 PM–8:00 PM. Add these to your calendar?";

    const md = structureReply(dense);
    expect(md).toContain("**Work**");
    expect(md).toContain("**Sleep**");
    expect(md).toContain("**Best free times**");
    expect(md).toContain("- on Mon 10 Aug, 5:55 PM–6:55 PM");
    expect(md).toContain("**Found 5 tasks**");
    expect(md).toContain("- groom");
    expect(md).toContain("**Proposed 3 blocks**");
    expect(md).toContain("Easy task 2 — Mon 10 Aug, 6:50 PM–7:20 PM");
    expect(md).toContain("Add these to your calendar?");

    const { container } = render(<FormattedText text={dense} />);
    expect(container.querySelectorAll("ul").length).toBeGreaterThanOrEqual(3);
    expect(container.querySelectorAll("ul li").length).toBeGreaterThanOrEqual(9);
  });

  it("leaves already-structured markdown alone", () => {
    const md = "## Plan\n- one\n- two";
    expect(structureReply(md)).toBe(md);
  });

  it("keeps a look-style week reply as a list", () => {
    const look =
      "**Work** — weekdays 8:45 AM–4:45 PM\n\n**7 on your calendar**\n- Study on Mon 10 Aug, 4:45 PM–5:45 PM\n- Gym on Thu 13 Aug, 4:45 PM–5:45 PM";
    expect(structureReply(look)).toBe(look);
    const { container } = render(<FormattedText text={look} />);
    expect(container.querySelectorAll("ul li")).toHaveLength(2);
    expect(container.querySelector("strong")?.textContent).toBe("Work");
  });
});
