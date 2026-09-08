<!--
	Whitespace-sensitive: inside <pre> every inter-tag byte is content, so a welded run has
	no whitespace boundary to break at. The one render-free break is inside tag syntax: a
	closing tag whose content ends on a visible byte splits and dangles its `>`
	(`</b⏎\t>tail`), the ws-sensitive family rule. At exactly 100 columns the run stays intact.
-->
<pre>
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <b>welded</b>tail stays intact at one hundred
</pre>

<!-- One char over: the closing `>` dangles to duck under the print width -->
<pre>
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <b>welded</b
	>tail stays intact at one hundred one
</pre>
