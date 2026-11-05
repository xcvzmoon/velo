import * as v from 'valibot';

export const diagnosticInputSchema = v.object({
  label: v.pipe(
    v.string(),
    v.trim(),
    v.check(
      (label) => Array.from(label).length >= 1 && Array.from(label).length <= 64,
      'Enter a label between 1 and 64 characters.',
    ),
  ),
});
