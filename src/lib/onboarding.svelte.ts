// Whether the onboarding is showing. Opens by itself on a fresh install (no library yet) and
// from More -> Onboarding guide; existing users are never shown it unasked.
import { api } from "./api";
import { settings } from "./settings.svelte";

class OnboardingStore {
  open = $state(false);

  /** Call once settings are loaded. */
  async checkFirstRun() {
    if (settings.values.onboarded) return;
    const library = await api.libraryList().catch(() => []);
    if (library.length) settings.set("onboarded", true); // already set up before onboarding existed
    else this.open = true;
  }

  finish() {
    settings.set("onboarded", true);
    this.open = false;
  }
}

export const onboarding = new OnboardingStore();
