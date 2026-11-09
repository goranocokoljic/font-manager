<script setup lang="ts">
import { setMotionSpeed, setReducedMotion, type MotionSpeed, type ReducedMotion } from './speed';

/**
 * Fades its one child in and out. Other `<Transition>` props (`mode`, `appear`) pass through.
 * `reduced`: under reduced motion, `cut` (the default) or the reduced `fade`.
 */
const props = withDefaults(defineProps<{ speed?: MotionSpeed; reduced?: ReducedMotion }>(), {
  speed: 'move',
  reduced: 'cut',
});

function prepare(el: Element): void {
  setMotionSpeed(el, props.speed);
  setReducedMotion(el, props.reduced);
}
</script>

<template>
  <Transition name="fm-fade" @before-enter="prepare" @before-leave="prepare">
    <slot />
  </Transition>
</template>

<!-- Not scoped: the classes land on the slotted child, which belongs to the parent. -->
<style>
.fm-fade-enter-active,
.fm-fade-leave-active {
  transition: opacity var(--fm-transition-time) var(--fm-ease);
}
.fm-fade-enter-from,
.fm-fade-leave-to {
  opacity: 0;
}
</style>
