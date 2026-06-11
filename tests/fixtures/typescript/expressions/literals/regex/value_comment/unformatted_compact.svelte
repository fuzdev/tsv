<script>
// block comment after a regex value, before a sibling property
const obj1={a:/re//* c */,b:1};

// consecutive block comments after a regex value (the collection loop must take both)
const obj2={a:/re//* c1 *//* c2 */,b:1};

// block comment after the sole regex value, before the closing brace
const obj3={a:/re//* c */};

// block comment after a flagged regex value (pattern/flags split, then comment)
const obj4={a:/re/g/* c */,b:1};

// block comment after a regex literal starting with `=` (the /= relex branch)
const obj5={a:/=re//* c */,b:1};

// block comment after a regex call argument, before the closing paren
fn(/re//* c */);

// block comment after a regex array element, before the closing bracket
const arr=[/re//* c */];

// multiline block comment after a regex value (sets the line-terminator flag)
const obj6={a:/re//* multi
	line */,b:1};
</script>
