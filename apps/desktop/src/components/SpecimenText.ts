// A specimen's text with tofu (E43.4): the characters in `missing` are boxes, an
// element each with `role="img"` and the name "missing glyph ž", so a character the
// face lacks is never drawn in another font. The rest is plain text, so it inherits
// the parent's face, size, tracking and outline. A render function: a template would
// put whitespace between the runs.
import { computed, defineComponent, h, type PropType } from 'vue';
import { specimenRuns, TOFU_CLASS, tofuLabel } from '../preview/tofu';

export default defineComponent({
  name: 'SpecimenText',
  props: {
    text: { type: String, required: true },
    /** The characters to draw as tofu. */
    missing: { type: Array as PropType<readonly string[]>, default: () => [] },
  },
  setup(props) {
    const runs = computed(() => specimenRuns(props.text, props.missing));
    return () => {
      // Without tofu the text is one text node, as before: a fragment would put empty
      // anchor nodes around it, and the first child of the line would be one of them.
      const only = runs.value.length === 1 ? runs.value[0]! : undefined;
      if (only && !only.missing) return only.text;
      return runs.value.map((run) =>
        run.missing
          ? h('span', {
              class: TOFU_CLASS,
              role: 'img',
              'aria-label': tofuLabel(run.text),
              'data-char': run.text,
            })
          : run.text,
      );
    };
  },
});
