import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { getMDXComponents } from "@/components/mdx";
import { legalSource } from "@/lib/source";

export default async function LegalPage(props: PageProps<"/legal/[slug]">) {
  const { slug } = await props.params;
  const page = legalSource.getPage([slug]);
  if (!page) notFound();
  const MDX = page.data.body;
  return (
    <article className="lp-legal-body">
      <h1>{page.data.title}</h1>
      <p className="lp-legal-updated">Last updated {page.data.updated}</p>
      <MDX components={getMDXComponents()} />
    </article>
  );
}

// eslint-disable-next-line @typescript-eslint/require-await
export async function generateStaticParams() {
  return legalSource.getPages().map((p) => ({ slug: p.slugs[0] }));
}

export async function generateMetadata(props: PageProps<"/legal/[slug]">): Promise<Metadata> {
  const { slug } = await props.params;
  const page = legalSource.getPage([slug]);
  if (!page) return {};
  return { title: page.data.title, description: page.data.description, alternates: { canonical: `${page.url}/` } };
}
