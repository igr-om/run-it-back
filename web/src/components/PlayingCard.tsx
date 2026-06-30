const SUIT_CHAR: Record<string, string> = { c: "\u2663", d: "\u2666", h: "\u2665", s: "\u2660" };
const SUIT_CLASS: Record<string, string> = { c: "suit-c", d: "suit-d", h: "suit-h", s: "suit-s" };

/** Renders a single card from shorthand like "Ah", "Td", "2c". */
export default function PlayingCard({ code }: { code: string }) {
  const rank = code.slice(0, -1);
  const suit = code.slice(-1).toLowerCase();
  const suitClass = SUIT_CLASS[suit] ?? "";
  const suitChar = SUIT_CHAR[suit] ?? suit;
  return (
    <div className={"playing-card " + suitClass}>
      {rank}
      {suitChar}
    </div>
  );
}
