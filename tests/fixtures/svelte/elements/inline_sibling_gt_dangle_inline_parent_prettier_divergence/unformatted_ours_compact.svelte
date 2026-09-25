<!-- An inline element glued to a following {#await} (or to a {#snippet} glued on both sides) INSIDE an inline parent — a component or an inline element: when the block renders multiline, the element's closing > dangles onto the block-head line (</span⏎>{#await…}), exactly as it does inside a block parent. Prettier keeps </span>{#await…} hugged. -->

<!-- a nested <style> forces the {#await} body open, inside a component -->
<Comp><span>text</span>{#await promise}<style>.class{color:red;}</style>{/await}</Comp>

<!-- a multi-line comment forces it open, inside an inline element -->
<span><span>text</span>{#await promise then value}text<!-- comment1
comment2 -->{/await}</span>

<!-- width: the hugged line overflows, but the block fits on the fresh line the dangle opens, so it stays inline there -->
<a href="value"><span>text</span>{#await promise}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} />{/await}</a>

<!-- width: the block overflows even on the fresh line, so its body drops too -->
<a href="value"><span>text</span>{#await promise}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5} />{/await}</a>

<!-- a {:then} branch forces it open (a <pre>), after text and a space before the element -->
<Comp>text <span>text</span>{#await promise}text{:then value}<pre>text1
text2</pre>{/await}</Comp>

<!-- a {:catch} branch forces it open (a nested <style>) -->
<span><span>text</span>{#await promise}text{:catch error}<style>.class{color:red;}</style>{/await}</span>

<!-- a {#snippet} glued to content on both sides stays in the dangle regime -->
<Comp><span>text</span>{#snippet fn()}<style>.class{color:red;}</style>{/snippet}tail</Comp>

<!-- a glued run: only the run's last closing > dangles -->
<Comp><span>inline1</span><span>inline2</span>{#await promise}<style>.class{color:red;}</style>{/await}</Comp>

<!-- control: a short {#await} that stays inline keeps the > hugged and the parent inline -->
<Comp><span>text</span>{#await promise}text{/await}</Comp>

<!-- control: a short {#snippet} glued on both sides that stays inline keeps the > hugged too -->
<Comp><span>text</span>{#snippet fn2()}text{/snippet}tail</Comp>
