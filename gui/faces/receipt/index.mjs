// SPDX-License-Identifier: Apache-2.0
// The one door. A shell mounts this and learns nothing else about the face; the face
// learns nothing at all about the shell.

export {
  mount, face, createFace, callerFor, toRecord, digestAgreement, RECEIPT_MESSAGES,
} from './receipt.mjs';
export { DECLARATION, FACE_ID, QUESTION } from './declaration.mjs';
