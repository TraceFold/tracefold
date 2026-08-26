// SPDX-License-Identifier: Apache-2.0
// A placeholder face. It imports nothing.
//
// That is the point of it rather than a shortcut in it: everything this face is allowed
// to touch arrives as an argument, so there is no second route to the network for it to
// take and no way for it to learn which dock, space or tab it is standing in. The rebuilt
// faces are req/03's work; this one exists so the frame can be shown carrying something.

export const mount = (host, port, notices) => {
  const doc = host.ownerDocument;
  const box = doc.createElement('div');
  box.className = 'demo-face';

  const said = doc.createElement('p');
  said.className = 'demo-said';
  said.textContent = 'A bottom dock face. Its height is a shell act with an inverse, like every other size here.';

  const seen = doc.createElement('p');
  seen.className = 'demo-seen';
  seen.textContent = 'pointer entries: 0';

  box.append(said, seen);
  host.append(box);

  let count = 0;
  const onEnter = () => { count += 1; seen.textContent = `pointer entries: ${count}`; };
  host.addEventListener('pointerenter', onEnter);



  // The unmount is the contract. Without it nothing above can ever be released.
  return () => {
    host.removeEventListener('pointerenter', onEnter);
    box.remove();
  };
};
