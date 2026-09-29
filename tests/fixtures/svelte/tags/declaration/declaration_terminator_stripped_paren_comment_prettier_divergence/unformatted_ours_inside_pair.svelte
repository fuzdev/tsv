<!-- an own-line block, glued onto the value's line -->
{let b = x || (y
/* c1 */
)}

<!-- a hugged arrow's body: the comment rides out past the call -->
{let c = f(() => x || (y
/* c2 */
))}

{let d = f(() => x || (y // c3
))}

<!-- the comment flushes inside the value: nothing ends it on a comment, and no `;` -->
{let g = f(x || (y // c4
), 1)}

{let h = `${x + (y // c5
)}`}

<!-- the bare control: no comment, no `;` -->
{let q = x || y}
