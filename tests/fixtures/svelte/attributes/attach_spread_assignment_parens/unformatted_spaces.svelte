<!-- an assignment used as a value needs clarity parens, and the spread and attach heads
	supply them the same way every other braced value does -->
<div {...(  a  =  b  )} {@attach (  c  =  d  )}></div>

<!-- every other expression type needs none: a sequence and an object carry their own -->
<div {...(  e  ,  f  )} {@attach {  g  :  1  }}></div>
