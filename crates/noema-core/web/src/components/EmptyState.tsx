import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

const STARTERS = [
  "Say hello and tell me Noema is working.",
  "remember this: I prefer concise setup instructions",
  "What did you just remember?"
];

export function EmptyState({ onPick }: { onPick: (starter: string) => void }) {
  return (
    <div className="empty-state">
      <p className="eyebrow">Today</p>
      <h1>What should we work on?</h1>
      <p>
        Start with one chat. Memory and activity appear in the transcript when Noema has something worth showing.
      </p>
      <div className="starter-grid">
        {STARTERS.map((starter) => (
          <Card key={starter} className="starter-card">
            <CardContent>
              <Button type="button" variant="ghost" className="starter-button" onClick={() => onPick(starter)}>
                {starter}
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
