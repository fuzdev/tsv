<style>
.class1{padding:1rem;

/* Modern pseudo-class in nesting */
&:user-valid{border-color:green;}

/* Modern pseudo-element in nesting */
&::highlight(search){background:yellow;}

/* At-rule nesting with modern features */
@media (width >= 768px){& > .class2{font-size:2rem;}}}
</style>
