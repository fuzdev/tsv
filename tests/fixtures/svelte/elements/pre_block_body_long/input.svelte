<!-- A long block body inside <pre> does NOT drop to its own line: white-space is significant in <pre>, so the body stays on the pre line even past printWidth — the uniform body-drop is gated off. Both formatters agree. Outside <pre> the same body drops (see blocks/if/element_body_long). -->
<pre>{#if cond}this is a long run of plain text content that goes well beyond the hundred column limit here{/if}</pre>

<pre>{#each items as item}another long run of plain text content well beyond the hundred column limit goes here{/each}</pre>
