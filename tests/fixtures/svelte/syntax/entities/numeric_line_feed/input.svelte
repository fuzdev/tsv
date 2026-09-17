<!--
	A numeric reference to LINE FEED decodes to a line feed inside an attribute value: every
	spelling, beside an expression, on a component prop and on a directive. In text it decodes
	to a space.
-->
<p data-attr="a&#10;b">text1</p>
<p data-attr="a&#x0A;b&#X0a;c&#10d">text2</p>
<p data-attr="a&#10;{b}&#10;c">text3</p>
<Comp prop="a&#10;b" />
<p style:color="a&#10;b">text4</p>

<p>a&#10;b &#x0A;c</p>
