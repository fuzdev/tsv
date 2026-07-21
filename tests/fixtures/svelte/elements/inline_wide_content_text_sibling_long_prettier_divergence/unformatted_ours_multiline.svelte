<!--
	A wide inline element whose content overflows, followed by text and then ANOTHER inline
	element. tsv wraps the content within print width (the printWidth-hard-limit divergence);
	the text between (`mid`) takes its OWN line after the dangled `>`. Unlike the terminal case
	(inline_wide_content_trailing_long, which hugs a space-authored tail), non-terminal text
	stays on its own line — hugging it is non-convergent (it shifts the following element's fit).
	prettier agrees on the `mid` line, so the divergence is purely the content wrap.
-->
<p>
	<a href="#">alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar</a> mid <b>x</b>
</p>
