import * as stylex from "@stylexjs/stylex";
import { ShellSidebar } from "@/components/shell/ShellSidebar";
import type { ShellMenuItem, ShellMenuLevel } from "@/components/shell/shellNavigation";
import { ProjectManagerPane } from "./ProjectManagerPane";
import type { WorkProject } from "./workTypes";

export function WorkSidebar({
  menuLevel,
  projects,
  onSelectItem,
  onUpdated
}: {
  menuLevel: ShellMenuLevel;
  projects: readonly WorkProject[];
  onSelectItem: (item: ShellMenuItem) => void;
  onUpdated: () => void | Promise<void>;
}) {
  return (
    <div data-slot="work-sidebar" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.navigation)}>
        <ShellSidebar menuLevel={menuLevel} onSelectItem={onSelectItem} />
      </div>
      <ProjectManagerPane projects={projects} onUpdated={onUpdated} />
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    height: "100%",
    minHeight: 0,
    flexDirection: "column",
    overflow: "hidden"
  },
  navigation: {
    minHeight: 0,
    flex: 1
  }
});
