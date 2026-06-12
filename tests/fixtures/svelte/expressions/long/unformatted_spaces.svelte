<!-- Template expression wrapping -->

<!-- Function call expressions -->

<!-- 100 chars - stays inline (at element boundary) -->
<p>  {
fn(argument1, argument2, argument3, argument4, argument5, argument6, argument7, argument8a)}  </p>

<!-- 101 chars - element goes to block mode, call stays inline -->
<p>  {fn(
  argument1, argument2, argument3, argument4, argument5, argument6, argument7, argument8ab)}  </p>

<!-- Call at 100 chars (with indent) - stays inline -->
<p>  {fn(
  argument1, argument2, argument3, argument4, argument5, argument6, argument7, argument8abcdef)}  </p>

<!-- Call at 101 chars (with indent) - args wrap -->
<p>  {fn(
  argument1,
  argument2,
  argument3,
  argument4,
  argument5,
  argument6,
  argument7,
  argument8abcdefg,
  )}  </p>

<!-- Ternary expressions -->

<!-- 100 chars - stays inline (at element boundary) -->
<p>  {
conditionValueThatIsVeryLongAndExceedsBoundaryAAAAAAAAAAAA ? resultForTrue : resultForFalse}  </p>

<!-- 101 chars - element goes to block mode, ternary stays inline -->
<p>  {conditionValueThatIsVeryLongAndExceedsBoundaryAAAAAAAAAAAAB ? resultForTrue : resultForFalse}  </p>

<!-- Ternary at 100 chars (with indent) - stays inline -->
<p>  {
conditionValueThatIsVeryLongAndExceedsBoundaryAAAAAAAAAAAAAAAAx ? 
resultForTrue 
:
 resultForFalse
 }  </p>

<!-- Ternary at 101 chars (with indent) - branches wrap -->
<p>  {conditionValueThatIsVeryLongAndExceedsBoundaryAAAAAAAAAAAAAAAAxy
  ? resultForTrue
  : resultForFalse}  </p>

<!-- Logical expressions -->

<!-- 100 chars - stays inline (at element boundary) -->
<p>  {condA && condB && condC && condD && condE && condF && condG && condH && condI && condJJJJJJ}  </p>

<!-- 101 chars - element goes to block mode, logical stays inline -->
<p>  {condA && condB && condC && condD && condE && condF && condG && condH && condI && condJJJJJJJ}  </p>

<!-- Logical at 100 chars (with indent) - stays inline -->
<p>  {
condA 
&& condB 
&& condC && condD && condE && condF && condG && condH && condI && condJJJJJJJJJJK
}  </p>

<!-- Logical at 101 chars (with indent) - chain wraps -->
<p>  {condA &&
  condB &&
  condC &&
  condD &&
  condE &&
  condF &&
  condG &&
  condH &&
  condI &&
  condJJJJJJJJJJKL}  </p>
