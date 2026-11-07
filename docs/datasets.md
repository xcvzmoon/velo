# Generate and import test data

Open **Tests → Test data**. Choose Faker, CSV, or JSON, configure the recipe, then select **Apply data recipe**. A saved test contains the recipe, never the prepared records or an imported filename. Imported files must be selected again after loading a test. Keep sensitive literal values out of recipes because recipes are saved and exported as plaintext.

Select **Prepare and preview records** to inspect the first five rows without network traffic. Preview includes intentional invalid and boundary values. Clear or leave the editor to discard the preview. **Review load** prepares the complete dataset again, resolves every request, checks budgets, and shows the target and allocation settings before authorization. Preparation failures send no requests. **Cancel data preparation** discards generated rows and signals native resolution to stop between records. A bounded native normalization already in progress finishes before its cancelled result is discarded. No temporary files are written.

## Faker recipes

Faker is pinned to **10.6.0**. Supported locales are `en`, `de`, and `fr`; German and French use English fallback for missing locale data. The seed is an unsigned 32-bit integer. Generation starts from a fresh Faker instance for each preparation. The same version, seed, locale, record count, and ordered fields reproduce the same records. A seed does not reproduce concurrent worker assignment or network completion order.

Use **Add a generated field** to select a generator and its arguments. The ordered JSON field list supports editing and removing fields. Applying the recipe rejects unknown identifiers, unknown arguments, duplicate names, forward references, and invalid ranges. No user JavaScript is evaluated.

| Generator    | Arguments                   | Behavior                                                        |
| ------------ | --------------------------- | --------------------------------------------------------------- |
| `first_name` | None                        | `person.firstName()`                                            |
| `last_name`  | None                        | `person.lastName()`                                             |
| `uuid`       | None                        | `string.uuid()`                                                 |
| `email`      | `first`, `last`, `provider` | Uses earlier name fields and an explicit provider domain        |
| `integer`    | `min`, `max`                | Inclusive signed 32-bit integer range                           |
| `literal`    | `value`                     | Explicit string, including empty and invalid application values |
| `copy`       | `field`                     | Copies an earlier field in the same record                      |

Related fields are evaluated in order, within each record. Email fields use the referenced first and last names. Copy fields consume no random draw. Literal values make validation tests intentional; they are shown unchanged in previews. Unique record allocation does not guarantee that Faker generates distinct field values.

Generated login identities require corresponding accounts on the target. Generation does not create or remove remote records.

## Import CSV and JSON

Choose a source and provide a JSON mapping whose keys are request variable names and whose values are source column names, for example `{"userId":"id","email":"email_address"}`. Select a local file after applying the recipe.

CSV uses commas, a nonempty unique header row, standard quoting, and UTF-8 text. Quoted commas and multiline values are supported; blank rows are ignored. Every row must have the header's column count. JSON must be an array of objects with string, finite number, or boolean values. Nulls, arrays, and nested objects are unsupported, including in unmapped fields. JSON numbers use JavaScript number precision; import large identifiers as strings. Both formats convert mapped values to strings and reject missing mapped columns. Errors identify the row or field without echoing cell values.

## Bind fields to requests

Use the existing `{{name}}` syntax in URLs, ordered headers/query pairs, authentication, text, JSON, form, and multipart metadata. Data fields override environment and shared variables. JSON escaping and URL encoding follow the existing [authoring rules](authoring.md#environments-and-variables). Upload paths still require local selection and do not accept substitutions. Every prepared URL must have the reviewed origin.

Dataset cell values are literal. A cell containing `{{secret:name}}` does not retrieve a secret, and a cell containing `{{other}}` does not evaluate another template. Keep OS secret references in the definition's request fields or shared/environment values.

## Lifetime, allocation, and exhaustion

Lifetime controls when a record is selected. It is independent of the selection order.

| Lifetime    | Reuse                                                                                   |
| ----------- | --------------------------------------------------------------------------------------- |
| `run`       | All workers share one selected record for the whole run                                 |
| `user`      | Each worker keeps its record until retirement; replacement workers allocate new records |
| `iteration` | Each logical request allocates a record; redirects retain that request's values         |

Arrival workloads support `run` and `iteration`. They reject `user` because arrival tasks have no persistent user identity.

Sequential allocation traverses input order. Unique allocation also traverses input order and never recycles a record index. Random allocation uses a native seeded Fisher-Yates shuffle with SplitMix64, visiting each index once per cycle. Repeat cycles replay the same permutation. The global allocation cursor advances under the admission lock, so concurrent workers cannot allocate the same index within a cycle. Completion order and the worker receiving an index are unspecified. A run or user lifetime deliberately reuses its selected record without another allocation.

| Exhaustion policy | Outcome                                                                               |
| ----------------- | ------------------------------------------------------------------------------------- |
| `stop`            | Close admission, drain in-flight requests, finish as completed with `exhausted: true` |
| `repeat`          | Start another cycle and record the repeat count; forbidden with unique allocation     |
| `fail`            | Close admission, drain, finish as failed with `DATASET_EXHAUSTED`                     |

Exhaustion is detected when another allocation is requested after the last record. Consuming the final record alone does not mark the dataset exhausted. Explicit cancellation retains partial counts and follows the existing drain grace.

## Limits and retained metadata

Recipes are limited to 64 KiB, 64 fields, and 10,000 records. Imported sources and prepared rows are limited to 8 MiB. Resolved request templates and worker clones must fit the configured request budget and a 16 MiB dataset ceiling. Normalized request sizes, template overhead, and upload buffers count toward native preparation budgets. These safeguards do not describe the entire process's memory use; JSON parsing, Faker locale data, transport pools, and runtime overhead also use memory.

Rust retains immutable prepared request templates only while executing. Generation, import parsing, interpolation, and validation finish before measured traffic. The request loop selects a prepared template; it never calls JavaScript or generates values. Templates, upload readers, and temporary preparation allocations are released on completion, failure, or cancellation. The frontend clears resolved payloads after a start attempt, and imported content and previews when load admission begins or the editor unmounts.

Run confirmation and history retain source kind, Faker version and locale where applicable, seed, prepared record count, lifetime, allocation, exhaustion policy, allocation count, repeat cycles, and exhaustion state. They omit field definitions, mapping names, literal values, source filenames, and rows. Arrival metrics separately count slots rejected by dataset exhaustion as `dataset_drops`; those slots do not enter HTTP failure rates. Final summaries show dataset outcomes alongside ordinary request metrics. Saved test recipes provide the generator configuration needed to explain reproducibility.
