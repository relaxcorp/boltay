import { listen } from "@tauri-apps/api/event";
import { api, needed, type ModelId, type Models, type Speech } from "./api";

/// Model files on disk and downloads in flight, kept live by backend events.
class ModelsState {
  data = $state<Models | null>(null);
  progress = $state<Partial<Record<ModelId, number>>>({});
  errors = $state<Partial<Record<ModelId, string>>>({});
  #started = false;

  async refresh() {
    this.data = await api.models();
  }

  start() {
    if (this.#started) return;
    this.#started = true;
    this.refresh();
    listen<{ id: ModelId; done: number; total: number }>("model-progress", (e) => {
      this.progress[e.payload.id] = e.payload.done / e.payload.total;
    });
    listen<{ id: ModelId; error: string | null }>("model-done", (e) => {
      delete this.progress[e.payload.id];
      if (e.payload.error) this.errors[e.payload.id] = e.payload.error;
      this.refresh();
    });
    listen("engine-changed", () => this.refresh());
  }

  item(id: ModelId) {
    return this.data?.items.find((m) => m.id === id);
  }

  downloading(id: ModelId): boolean {
    return this.data?.downloading.includes(id) || id in this.progress;
  }

  /// Share of the file on disk, 0..1: live progress, or what a previous attempt left.
  share(id: ModelId): number {
    const m = this.item(id);
    return this.progress[id] ?? (m ? m.done / m.size : 0);
  }

  ready(speech: Speech): boolean {
    return needed(speech).every((id) => this.item(id)?.ready);
  }

  /// Bytes still to fetch for a language.
  missingBytes(speech: Speech): number {
    return needed(speech)
      .map((id) => this.item(id))
      .reduce((sum, m) => sum + (m && !m.ready ? m.size - m.done : 0), 0);
  }

  async download(ids: ModelId[]) {
    for (const id of ids) {
      if (this.item(id)?.ready) continue;
      delete this.errors[id];
      this.progress[id] = this.share(id);
      await api.download(id);
    }
  }

  async cancel(id: ModelId) {
    await api.cancelDownload(id);
  }

  async remove(id: ModelId) {
    await api.deleteModel(id);
    await this.refresh();
  }
}

export const models = new ModelsState();
