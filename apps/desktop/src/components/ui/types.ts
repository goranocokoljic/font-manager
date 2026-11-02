// Types the `ui/` components share with the views that use them.

/** One choice of a `UiSelect`. */
export interface SelectOption<T extends string | number = string> {
  value: T;
  label: string;
  disabled?: boolean;
  /** A tooltip on the option (why it is disabled, for one). */
  title?: string;
}
