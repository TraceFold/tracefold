// SPDX-License-Identifier: Apache-2.0
// One request over the wire, and the classification of what came back.
//
// The classification is the point. The crate answers every refusal in
// application/problem+json (gx-api/src/problem.rs:381-395), so an HTTP error without
// that media type did not come from the engine — something between us wrote it. Those
// are different facts and they get different names, because a caller that cannot tell
// "the gate said no" from "nothing reached the gate" will retry the first one.

import {
  OUTCOME, FAILURE, PROBLEM_MEDIA_TYPE, answered, refused, failed,
} from './wire.mjs';

function mediaTypeOf(response) {
  const raw = response.headers?.get?.('content-type') ?? '';
  return String(raw).split(';')[0].trim().toLowerCase();
}

/**
 * @param {Function} fetchImpl
 * @param {{url:string, verb:string, headers:object, body:string|null, expects:'json'|'stream'}} request
 */
export async function send(fetchImpl, request) {
  let response;
  try {
    response = await fetchImpl(request.url, {
      method: request.verb,
      headers: request.headers,
      body: request.body,
    });
  } catch (cause) {
    return failed(FAILURE.TRANSPORT, null, String(cause?.message ?? cause));
  }

  const status = response.status;
  const mediaType = mediaTypeOf(response);

  if (mediaType === PROBLEM_MEDIA_TYPE) {
    const text = await response.text();
    try {
      return refused(status, JSON.parse(text));
    } catch (cause) {
      return failed(FAILURE.UNDECODABLE, status, `problem+json did not parse: ${cause?.message ?? cause}`);
    }
  }

  if (status >= 400) {
    return failed(
      FAILURE.UNEXPECTED_MEDIA_TYPE,
      status,
      `HTTP ${status} carried ${mediaType || 'no media type'}; the engine states refusals in ${PROBLEM_MEDIA_TYPE}`,
    );
  }

  if (request.expects === 'stream') {
    return answered(status, null, { stream: response.body ?? null });
  }

  const text = await response.text();
  try {
    return answered(status, JSON.parse(text));
  } catch (cause) {
    // A body that will not parse is not a body. Handing back a partial object here is
    // exactly the silent wrong answer the audits grade as the worst kind.
    return failed(FAILURE.UNDECODABLE, status, `body did not parse: ${cause?.message ?? cause}`);
  }
}

export { OUTCOME };
