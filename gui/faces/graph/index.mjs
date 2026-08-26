// SPDX-License-Identifier: Apache-2.0
// The one door. A shell mounts this and learns nothing else about the face; the face
// learns nothing at all about the shell.

export {
  mount, face, createFace, callerFor, toRecord, GRAPH_MESSAGES,
} from './graph.mjs';
export { DECLARATION, FACE_ID, QUESTION } from './declaration.mjs';
