<!--
	A wide self-closing inline component with several attributes drops to its own line where
	all its attributes still fit on one line; the preceding word (`and`) stays hugged. Pins
	that the flow boundary keys on the component being too wide for the text line, not on
	whether its own attributes could break.
-->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 aaaaaaaaaaaaaa and <Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={v5} /> tail1 tail2 tail3
</p>
