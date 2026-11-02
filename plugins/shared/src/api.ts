// The one place that reaches into the generated contract types
// (`pnpm gen:types`): the plugins never keep copies of their own.
export { API_VERSION } from '../../../apps/desktop/src/api/types';
export type {
  ActivationOutcome,
  ApiError,
  ApiVersion,
  Conflict,
  ConflictPolicy,
  ConflictWith,
  DeactivateRequest,
  DocumentClosedRequest,
  DocumentClosedResponse,
  ErrorCode,
  FaceId,
  FetchedFont,
  FontSwitchPayload,
  FontsNeededRequest,
  FontsNeededResponse,
  NeededFont,
  NeededFontMatch,
  OfferedFace,
  OutcomeStatus,
  Target,
} from '../../../apps/desktop/src/api/types';
