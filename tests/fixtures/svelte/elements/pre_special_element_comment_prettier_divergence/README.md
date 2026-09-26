# pre_special_element_comment_prettier_divergence

A `//` in the head of a special element inside `<pre>`. The head is the whitespace-sensitive
family's, so a `//` takes the same two shapes it takes on a `<pre>` or a component there:

- inside the `this={…}` binding, the comment carries a line break inside the braces, so the list
  wraps and the `>` hugs the `}`;
- ending the attribute list, the `>` cannot share the comment's line, so it takes the next line
  one level in, and the closing tag dangles its `>`.

Every break lands inside a tag, so the text inside `<pre>` is untouched.

- `input.svelte` — tsv's form.
- `unformatted_ours_compact.svelte` — each list on the tag-name line; tsv normalizes it to
  `input.svelte`.
- `output_prettier.svelte` — prettier deletes the comment inside the `this` binding, and moves a
  list-ending `//` into the closing tag (`</svelte:element // c`), which Svelte's parser rejects.

## Reason

Content preservation and a prettier bug: a comment is content, and prettier's output for the
list-ending case does not parse. See the `//` entry in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes)
and "Trailing comments in `{...}`" in the same section.
