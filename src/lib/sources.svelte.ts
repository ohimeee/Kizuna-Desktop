// Installed sources, shared across screens.
import { api, type InstalledSource, type SourceInfo } from "./api";

class SourcesStore {
  installed = $state<InstalledSource[]>([]);
  loaded = $state(false);

  get valid(): SourceInfo[] {
    return this.installed.filter((s): s is InstalledSource & SourceInfo => !!s.id) as SourceInfo[];
  }

  byId(id: string): SourceInfo | undefined {
    return this.valid.find((s) => s.id === id);
  }

  async refresh() {
    this.installed = await api.sourcesInstalled();
    this.loaded = true;
  }
}

export const sources = new SourcesStore();
