// SPDX-License-Identifier: Apache-2.0
// The one place a URL is assembled.
//
// Escaping an id at every call site is a rule somebody forgets once; assembling the
// address in a single function is a place where forgetting has nowhere to happen.
// The base path is here too, because a route reached without it answers 404 and the
// 404 looks like "the server does not have this" rather than "we addressed it wrong".

/** The prefix the crate nests its whole router under (gx-api/src/lib.rs:80). */
export const BASE_PATH = '/v1';

const PARAM = /^\{(.+)\}$/;

/**
 * Fill a path template with its parameters, escaping each one.
 * Throws on a missing parameter and on a parameter the template does not have:
 * a silently ignored argument is an argument that looks like it worked.
 */
export function fillPath(template, params = {}) {
  const wanted = new Set();
  const filled = template
    .split('/')
    .map((segment) => {
      const hit = PARAM.exec(segment);
      if (!hit) return segment;
      const name = hit[1];
      wanted.add(name);
      const value = params[name];
      if (value === undefined || value === null || value === '') {
        throw new TypeError(`path parameter "${name}" is required by ${template}`);
      }
      return encodeURIComponent(String(value));
    })
    .join('/');
  for (const given of Object.keys(params)) {
    if (!wanted.has(given)) throw new TypeError(`${template} has no path parameter "${given}"`);
  }
  return filled;
}

/** origin + base path + filled template + query. */
export function buildUrl(origin, template, params = {}, query = {}) {
  const root = String(origin).replace(/\/+$/, '');
  const path = fillPath(template, params);
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null) continue;
    search.append(key, String(value));
  }
  const tail = search.toString();
  return `${root}${BASE_PATH}${path}${tail ? `?${tail}` : ''}`;
}
