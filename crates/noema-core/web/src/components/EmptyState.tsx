import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

const STARTERS = [
  "Say hello and tell me Noema is working.",
  "remember this: I prefer concise setup instructions",
  "What did you just remember?"
];

export function EmptyState({ onPick }: { onPick: (starter: string) => void }) {
  return (
    <div className="grid min-h-[56vh] content-center justify-items-center gap-3.5 text-center">
      <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">Today</p>
      <h1 className="m-0 max-w-[680px] font-heading text-[clamp(38px,8vw,66px)] leading-none tracking-normal text-foreground">
        What should we work on?
      </h1>
      <p className="m-0 max-w-[560px] text-lg leading-[1.55] text-muted-foreground">
        Start with one chat. Memory and activity appear in the transcript when Noema has something worth showing.
      </p>
      <div className="mt-2.5 grid w-[min(720px,100%)] grid-cols-3 gap-2.5 max-[760px]:grid-cols-1">
        {STARTERS.map((starter) => (
          <Card key={starter} className="min-h-[54px] p-0">
            <CardContent className="h-full p-0">
              <Button
                type="button"
                variant="ghost"
                className="h-full min-h-[54px] w-full justify-start px-3.5 py-2.5 text-left whitespace-normal text-foreground"
                onClick={() => onPick(starter)}
              >
                {starter}
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
