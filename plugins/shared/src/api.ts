// The one place that reaches into the generated contract types
// (`pnpm gen:types`): the plugins never keep copies of their own.
export { API_VERSION } from '../../../apps/desktop/src/api/types';
export type {
  ActivationOutcome,
  ApiError,
  ApiVersion,
  DeactivateRequest,
  DocumentClosedRequest,
  DocumentClosedResponse,
  ErrorCode,
  FaceId,
  FontsNeededRequest,
  FontsNeededResponse,
  NeededFont,
} from '../../../apps/desktop/src/api/types';
