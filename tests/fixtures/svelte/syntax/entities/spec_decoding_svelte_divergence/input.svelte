<!--
	Four character references the HTML5 spec decodes and Svelte's decoder does not: an
	uppercase hex marker, a zero code, a code point in a plane its decoder omits, and — in
	an attribute value only — a semicolon-less reference standing before an underscore.
-->
<p>uppercase hex: &#X41; &#X1F4A9;</p>
<p>zero code: &#0; &#00; &#x0;</p>
<p>omitted planes: &#x30000; &#xF0000; &#x10FFFF;</p>
<div data-attr="&AMP_ &COPY_1"></div>

<!-- The forms each contrasts with, where the two decoders agree -->
<p>lowercase hex: &#x41; &#x1F4A9;</p>
<p>nonzero code: &#1; &#x1;</p>
<p>admitted planes: &#x2FFFF; &#xE0001;</p>
<div data-attr="&AMP- &AMP=1"></div>
