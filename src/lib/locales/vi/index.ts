import apps from "./apps";
import devtools from "./devtools";
import disk from "./disk";
import path from "./path";
import projects from "./projects";
import settings from "./settings";
import shell from "./shell";
import snapshots from "./snapshots";
import startup from "./startup";
import wsl from "./wsl";

const vi: Record<string, string> = {
  ...shell,
  ...apps,
  ...disk,
  ...settings,
  ...devtools,
  ...projects,
  ...wsl,
  ...startup,
  ...snapshots,
  ...path,
};

export default vi;
