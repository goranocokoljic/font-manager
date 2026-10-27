// A JSON Schema checker for the keywords `manifest.schema.json` uses, so
// the test needs no validator package. A keyword it does not know is an
// error: the schema cannot pass by using one this file ignores.

type Schema = Record<string, unknown>;

const ANNOTATIONS = new Set(['$schema', '$id', 'title', 'description']);

function typeOf(value: unknown): string {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  if (Number.isInteger(value)) return 'integer';
  return typeof value;
}

/** The problems of `value` against `schema`, one line each; empty when it fits. */
export function validate(schema: Schema, value: unknown, at = '$'): string[] {
  const errors: string[] = [];
  for (const [keyword, rule] of Object.entries(schema)) {
    if (ANNOTATIONS.has(keyword)) continue;
    switch (keyword) {
      case 'type': {
        const t = typeOf(value);
        if (t !== rule && !(rule === 'number' && t === 'integer')) {
          errors.push(`${at}: is ${t}, not ${rule}`);
        }
        break;
      }
      case 'const':
        if (JSON.stringify(value) !== JSON.stringify(rule)) {
          errors.push(`${at}: must be ${JSON.stringify(rule)}`);
        }
        break;
      case 'enum':
        if (!(rule as unknown[]).some((r) => JSON.stringify(r) === JSON.stringify(value))) {
          errors.push(`${at}: must be one of ${JSON.stringify(rule)}`);
        }
        break;
      case 'pattern':
        if (typeof value === 'string' && !new RegExp(rule as string, 'u').test(value)) {
          errors.push(`${at}: does not match ${rule}`);
        }
        break;
      case 'minLength':
        if (typeof value === 'string' && value.length < (rule as number)) {
          errors.push(`${at}: shorter than ${rule}`);
        }
        break;
      case 'required':
        if (typeOf(value) === 'object') {
          for (const key of rule as string[]) {
            if (!(key in (value as object))) errors.push(`${at}: ${key} is missing`);
          }
        }
        break;
      case 'properties':
        if (typeOf(value) === 'object') {
          for (const [key, sub] of Object.entries(rule as Record<string, Schema>)) {
            const v = (value as Record<string, unknown>)[key];
            if (v !== undefined) errors.push(...validate(sub, v, `${at}.${key}`));
          }
        }
        break;
      case 'additionalProperties':
        if (rule !== false) throw new Error(`${at}: only additionalProperties: false is checked`);
        if (typeOf(value) === 'object') {
          const known = Object.keys((schema.properties ?? {}) as object);
          for (const key of Object.keys(value as object)) {
            if (!known.includes(key)) errors.push(`${at}: ${key} is not allowed`);
          }
        }
        break;
      case 'items':
        if (Array.isArray(value)) {
          value.forEach((v, i) => errors.push(...validate(rule as Schema, v, `${at}[${i}]`)));
        }
        break;
      case 'minItems':
        if (Array.isArray(value) && value.length < (rule as number)) {
          errors.push(`${at}: fewer than ${rule} items`);
        }
        break;
      case 'maxItems':
        if (Array.isArray(value) && value.length > (rule as number)) {
          errors.push(`${at}: more than ${rule} items`);
        }
        break;
      case 'contains':
        if (Array.isArray(value) && !value.some((v) => validate(rule as Schema, v).length === 0)) {
          errors.push(`${at}: no item fits ${JSON.stringify(rule)}`);
        }
        break;
      case 'allOf':
        for (const sub of rule as Schema[]) errors.push(...validate(sub, value, at));
        break;
      default:
        throw new Error(`${at}: the keyword ${keyword} is not checked by this test`);
    }
  }
  return errors;
}
