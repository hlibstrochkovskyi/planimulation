/** Complete schema-3 JSON, separate from the bounded manual inventory format. */
export const MAX_SEASONAL_CHECKPOINT_BYTES = 64 * 2 ** 20;
// The restore envelope contains the original JSON as an escaped string. Keep
// decimal tokens and signed zero intact instead of reserializing JS numbers.
export const MAX_SEASONAL_COMMAND_BYTES = 2 * MAX_SEASONAL_CHECKPOINT_BYTES + 1024;
