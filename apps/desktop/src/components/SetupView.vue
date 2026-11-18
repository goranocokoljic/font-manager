<script setup lang="ts">
// The first-run setup (board 13, E55.4; DESIGN.md "First-run setup"): in the main window in
// place of the library while a step is open. The left pane (648 px, --fm-bg) asks, the right
// one (--fm-surface) shows what the answer leads to. The steps and which are open come from
// the setup store (`setupStatus`, decided in fm-core); this file only lays them out. Focus
// moves to each step's heading (the step focuses it when it mounts, after the fade); reopened from Settings, the step names are links and a close
// button appears.
import { t, type MessageKey } from '../i18n';
import FmFade from '../motion/FmFade.vue';
import { useSetupStore } from '../stores/setup';
import AppLogo from './AppLogo.vue';
import SetupFoldersStep from './SetupFoldersStep.vue';
import SetupLibraryPane from './SetupLibraryPane.vue';
import UiButton from './ui/UiButton.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';

const setup = useSetupStore();

const stepName = (step: string) => t(`setup.steps.${step}` as MessageKey);

function onKey(e: KeyboardEvent): void {
  // Esc closes the setup only where it has a close button (reopened from Settings).
  if (e.key === 'Escape' && setup.again) {
    e.preventDefault();
    setup.close();
  }
}
</script>

<template>
  <main class="setup" :aria-label="t('setup.label')" data-testid="setup" @keydown="onKey">
    <section class="ask">
      <header class="top">
        <AppLogo class="logo" />
        <UiIconButton
          v-if="setup.again"
          icon="close"
          :label="t('setup.close')"
          data-testid="setup-close"
          @click="setup.close()"
        />
      </header>

      <nav class="indicator" :aria-label="t('setup.stepsLabel')">
        <ol data-testid="setup-steps">
          <li
            v-for="(step, i) in setup.steps"
            :key="step"
            :class="{ done: i < setup.index, current: i === setup.index }"
            :aria-current="i === setup.index ? 'step' : undefined"
            :data-step="step"
          >
            <span class="track" aria-hidden="true" />
            <button
              v-if="setup.again"
              type="button"
              class="name link"
              :aria-label="t('setup.goToStep', { name: stepName(step) })"
              @click="setup.goTo(i)"
            >
              {{ stepName(step) }}
            </button>
            <span v-else class="name">{{ stepName(step) }}</span>
          </li>
        </ol>
      </nav>

      <FmFade mode="out-in">
        <div :key="setup.current" class="step">
          <SetupFoldersStep v-if="setup.current === 'fontFolders'" />
        </div>
      </FmFade>

      <p v-if="setup.error" class="error" role="alert">{{ setup.error }}</p>

      <footer class="actions">
        <UiButton
          v-if="setup.index > 0"
          class="back"
          data-testid="setup-back"
          @click="setup.back()"
        >
          <UiIcon name="arrowLeft" />
          {{ t('setup.back') }}
        </UiButton>
        <span class="spacer" />
        <UiButton v-if="setup.canSkip" class="skip" data-testid="setup-skip" @click="setup.skip()">
          {{ t('setup.skip') }}
        </UiButton>
        <button
          v-else
          type="button"
          class="primary"
          data-testid="setup-continue"
          @click="setup.next()"
        >
          {{ setup.last ? t('setup.finish') : t('setup.continue') }}
        </button>
      </footer>
    </section>

    <SetupLibraryPane v-if="setup.current === 'fontFolders'" class="show" />
  </main>
</template>

<style scoped>
.setup {
  display: grid;
  grid-template-columns: minmax(0, 648px) minmax(0, 1fr);
  height: 100vh;
  background-color: var(--fm-bg);
  color: var(--fm-fg);
}
.ask {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 40px 76px 32px;
  overflow-y: auto;
}
.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 28px;
}
.logo {
  height: 28px;
}
.indicator {
  margin-top: 72px;
}
/* Each step keeps the width of a third (the setup has three steps at most), so one or two
   steps do not stretch across the pane. */
.indicator ol {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.track {
  display: block;
  height: 2px;
  margin-bottom: var(--fm-space-3);
  border-radius: 1px;
  background-color: var(--fm-border-strong);
  transition: background-color var(--fm-motion-move) var(--fm-ease);
}
.done .track,
.current .track {
  background-color: var(--fm-accent);
}
.name {
  color: var(--fm-muted);
  font-size: var(--fm-text-md);
}
.current .name {
  color: var(--fm-fg);
  font-weight: 600;
}
.link {
  padding: 0;
  border: 0;
  background: none;
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}
.current .link {
  font-weight: 600;
}
.step {
  margin-top: 40px;
}
.error {
  margin: var(--fm-space-4) 0 0;
  color: var(--fm-danger);
  font-size: var(--fm-text-sm);
}
.actions {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  margin-top: auto;
  padding-top: var(--fm-space-6);
}
.spacer {
  flex: 1;
}
.back,
.skip {
  height: 32px;
  padding: 0 var(--fm-space-3);
  font-size: var(--fm-text-md);
}
.primary {
  height: 32px;
  padding: 0 var(--fm-space-4);
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font: inherit;
  font-weight: 600;
}
.show {
  min-height: 0;
}
</style>
