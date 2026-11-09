<script setup lang="ts">
import { setMotionSpeed, setReducedMotion, type MotionSpeed, type ReducedMotion } from './speed';

/**
 * Slides its one child in over a fade, and back out the same way. `from` is the side it
 * comes from: `start` and `end` follow the reading direction, so they mirror in
 * right-to-left chrome. Other `<Transition>` props (`mode`, `appear`) pass through.
 * `reduced`: under reduced motion, `cut` (the default) or the reduced `fade` without the slide.
 */
const props = withDefaults(
  defineProps<{
    from?: 'start' | 'end' | 'top' | 'bottom';
    /** A CSS length or percentage: the spec's page slide is 24px, its drawer 100%. */
    distance?: string;
    speed?: MotionSpeed;
    reduced?: ReducedMotion;
  }>(),
  { from: 'end', distance: '24px', speed: 'move', reduced: 'cut' },
);

function prepare(el: Element): void {
  const { style } = el as HTMLElement;
  const toward = props.from === 'start' || props.from === 'top' ? -1 : 1;
  const offset = `calc(${props.distance} * ${toward})`;
  const inline = props.from === 'start' || props.from === 'end';
  style.setProperty('--fm-slide-inline', inline ? offset : '0px');
  style.setProperty('--fm-slide-block', inline ? '0px' : offset);
  setMotionSpeed(el, props.speed);
  setReducedMotion(el, props.reduced);
}
</script>

<template>
  <Transition name="fm-slide" @before-enter="prepare" @before-leave="prepare">
    <slot />
  </Transition>
</template>

<!-- Not scoped: the classes land on the slotted child, which belongs to the parent. -->
<style>
.fm-slide-enter-active,
.fm-slide-leave-active {
  transition:
    transform var(--fm-transition-time) var(--fm-ease),
    opacity var(--fm-transition-time) var(--fm-ease);
}
.fm-slide-enter-from,
.fm-slide-leave-to {
  opacity: 0;
  transform: translate(var(--fm-slide-inline), var(--fm-slide-block));
}
[dir='rtl'] .fm-slide-enter-from,
[dir='rtl'] .fm-slide-leave-to {
  transform: translate(calc(var(--fm-slide-inline) * -1), var(--fm-slide-block));
}
</style>
