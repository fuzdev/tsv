<!-- A prettier-ignore'd inline element glued to a following block keeps its bytes AND its closing >: the block hugs it, expanding or not -->
<div>
	<!-- prettier-ignore --><span   data-attr="value"  >text1</span>{#if cond}
		<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />
	{/if}
</div>

<!-- short block: hugged and inline -->
<div>
	<!-- prettier-ignore --><span   data-attr="value"  >text2</span>{#if cond}text3{/if}
</div>
