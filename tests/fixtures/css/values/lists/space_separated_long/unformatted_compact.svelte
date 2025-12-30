<style>
div {
/* Short transform chain - stays inline (under 100 chars) */
transform: translateX(10px) rotate(45deg) scale(1.5);
/* Long transform chain - exceeds 100 chars, tests space-separated wrapping */
/* (100/101 char boundary tested in ../functions/transform_long_prettier_divergence) */
transform: translateX(0px) translateY(1px) rotate(0deg) scale(0) skewX(0deg) skewY(0deg) perspective(0px) rotateX(0deg) rotateY(0deg);
/* Long filter chain - exceeds 100 chars, tests space-separated wrapping */
filter: blur(0px) brightness(0) contrast(0) saturate(0) hue-rotate(0deg) drop-shadow(0 0 0 rgba(0,0,0,0));
}
</style>
