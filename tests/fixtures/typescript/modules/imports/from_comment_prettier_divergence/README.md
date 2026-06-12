# from_comment_prettier_divergence

A comment in the gap between an import's binding/specifiers and the `from` keyword is
preserved where the user placed it. Where Prettier relocates it depends on the binding
shape.

**Prettier** (`output_prettier.svelte`): a default/namespace binding keeps a block comment
in place but floats a line comment past the `;`; named specifiers pull the comment into the
braces (a block comment inline, a line comment expanding them):

```
import Foo /* c1 */ from './a';
import Bar from './b'; // c2
import * as ns1 /* c3 */ from './c';
import * as ns2 from './d'; // c4
import {a /* c5 */} from './e';
import {
	b, // c6
} from './f';
```

**tsv**: preserves each comment between the binding/specifiers and `from`:

```
import Foo /* c1 */ from './a';
import Bar // c2
from './b';
import * as ns1 /* c3 */ from './c';
import * as ns2 // c4
from './d';
import {a} /* c5 */ from './e';
import {b} // c6
from './f';
```

The default/namespace **block** comments (c1, c3) are dual-stable — both formatters keep
them in place. The default/namespace **line** comments (c2, c4) float to a statement-trailing
position in Prettier (mirroring `source_trailing_comment`), and the **named** comments
(c5, c6) relocate into the braces (mirroring `type_keyword_comment`). Per Comment Position
Philosophy.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
