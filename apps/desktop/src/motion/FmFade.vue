<script setup lang="ts">
import { setMotionSpeed, type MotionSpeed } from './speed';

/** Fades its one child in and out. Other `<Transition>` props (`mode`, `appear`) pass through. */
const props = withDefaults(defineProps<{ speed?: MotionSpeed }>(), { speed: 'move' });

const prepare = (el: Element) => setMotionSpeed(el, props.speed);
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
