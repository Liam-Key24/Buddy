import { describe, expect, it } from "vitest";
import { parseReplyBlocks, restoreStructure } from "../../lib/replyBlocks";

const FLAT =
  "**Work** — weekdays 8:45 AM–4:45 PM **7 on your calendar** - Study on Mon 10 Aug, 4:45 PM–5:45 PM - Study on Tue 11 Aug, 4:45 PM–5:45 PM - Study on Wed 12 Aug, 4:45 PM–5:45 PM - Gym on Thu 13 Aug, 4:45 PM–5:45 PM - Gym on Fri 14 Aug, 4:45 PM–5:45 PM - Climbing on Sat 15 Aug, 4:00 PM–6:00 PM - Climbing on Sun 16 Aug, 4:00 PM–6:00 PM";

describe("replyBlocks", () => {
  it("restores a flattened look reply into sections and a list", () => {
    const restored = restoreStructure(FLAT);
    expect(restored).toContain("\n\n**7 on your calendar**");
    expect(restored).toContain("\n- Study on Mon 10 Aug");

    const blocks = parseReplyBlocks(FLAT);
    expect(blocks[0]).toEqual({
      type: "fact",
      label: "Work",
      value: "weekdays 8:45 AM–4:45 PM",
    });
    const list = blocks.find((b) => b.type === "list");
    expect(list?.type).toBe("list");
    if (list?.type !== "list") return;
    expect(list.title).toBe("7 on your calendar");
    expect(list.items).toHaveLength(7);
    expect(list.items[0]).toContain("Study on Mon 10 Aug");
    expect(list.items[6]).toContain("Climbing on Sun 16 Aug");
  });

  it("parses already-lined markdown the same way", () => {
    const lined = `**Work** — weekdays 8:45 AM–4:45 PM

**7 on your calendar**
- Study on Mon 10 Aug, 4:45 PM–5:45 PM
- Gym on Thu 13 Aug, 4:45 PM–5:45 PM`;
    const blocks = parseReplyBlocks(lined);
    expect(blocks).toHaveLength(2);
    expect(blocks[1]).toMatchObject({
      type: "list",
      title: "7 on your calendar",
    });
    if (blocks[1].type === "list") expect(blocks[1].items).toHaveLength(2);
  });
});
