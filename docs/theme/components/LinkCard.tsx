export const LinkCard = ({ title, description, href }: { title: string; description: string; href: string }) => {
  return (
    <a
      href={href}
      className="p-4 border rounded-(--rp-radius) border-(--rp-c-text-3) hover:border-(--rp-c-text-0) transition-colors"
    >
      <span className="font-bold text-xl"> {title} </span>
      <p> {description} </p>
    </a>
  );
};
