import { Card } from "@astryxdesign/core/Card";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";

const STARTERS = [
  "Say hello and tell me Noema is working.",
  "remember this: I prefer concise setup instructions",
  "What did you just remember?"
];

export function EmptyState({ onPick }: { onPick: (starter: string) => void }) {
  return (
    <div {...stylex.props(styles.root)}>
      <p {...stylex.props(styles.eyebrow)}>Today</p>
      <h1 {...stylex.props(styles.title)}>
        What should we work on?
      </h1>
      <p {...stylex.props(styles.description)}>
        Start with one chat. Memory and activity appear in the transcript when Noema has something worth showing.
      </p>
      <div {...stylex.props(styles.starters)}>
        {STARTERS.map((starter) => (
          <Card key={starter} padding={0} minHeight={54}>
            <Button
              {...stylex.props(styles.starterButton)}
              type="button"
              variant="ghost"
              label={starter}
              onClick={() => onPick(starter)}
            >
              {starter}
            </Button>
          </Card>
        ))}
      </div>
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    minHeight: "56vh",
    alignContent: "center",
    justifyItems: "center",
    gap: 14,
    textAlign: "center"
  },
  eyebrow: {
    margin: 0,
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    letterSpacing: "0.12em",
    color: "var(--text-accent)",
    textTransform: "uppercase"
  },
  title: {
    margin: 0,
    maxWidth: 680,
    fontFamily: "var(--font-heading)",
    fontSize: "clamp(38px, 8vw, 66px)",
    lineHeight: 1,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  description: {
    margin: 0,
    maxWidth: 560,
    fontSize: 18,
    lineHeight: 1.55,
    color: "var(--muted-foreground)"
  },
  starters: {
    display: "grid",
    width: "min(720px, 100%)",
    gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
    gap: 10,
    marginTop: 10,
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr"
    }
  },
  starterButton: {
    width: "100%",
    height: "100%",
    minHeight: 54,
    justifyContent: "flex-start",
    padding: "10px 14px",
    textAlign: "left",
    whiteSpace: "normal",
    color: "var(--foreground)"
  }
});
