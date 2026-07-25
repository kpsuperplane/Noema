import * as stylex from "@stylexjs/stylex";
import { createFileRoute } from "@tanstack/react-router";
import { WorkSurface } from "@/components/work/WorkSurface";
import { ChatDetailRail } from "@/components/chatDetail/ChatDetailRail";
import { normalizeWorkSearch } from "@/components/work/workTypes";

export const Route = createFileRoute("/work/tasks/$taskId")({
  validateSearch: normalizeWorkSearch,
  component: WorkTaskDetailRoute
});

function WorkTaskDetailRoute() {
  const { taskId } = Route.useParams();
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const back = () => {
    void navigate({ to: "/work", search, replace: true });
  };
  return (
    <div {...stylex.props(styles.route)}>
      <div aria-label="Tasks context" {...stylex.props(styles.context)}>
        <WorkSurface
          search={search}
          onSearchChange={(next, replace) => void navigate({ search: next, replace })}
        />
      </div>
      <ChatDetailRail
        target={{ type: "task", taskId }}
        onClose={back}
        showWorkLink={false}
      />
    </div>
  );
}

const styles = stylex.create({
  route: {
    display: "grid",
    position: "relative",
    gridTemplateColumns: "minmax(0, 1fr) 440px",
    "--chat-detail-rail-width": "440px",
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    backgroundColor: "var(--noema-surface-card)",
    "@media (max-width: 979px)": {
      gridTemplateColumns: "1fr"
    }
  },
  context: {
    minWidth: 0,
    minHeight: 0,
    overflow: "hidden",
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "var(--noema-border-subtle)",
    "@media (max-width: 979px)": {
      display: "none"
    }
  }
});
