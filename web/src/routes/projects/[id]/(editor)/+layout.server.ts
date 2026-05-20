import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ parent, url }) => {
    const { project } = await parent();
    const role = project.user_role;
    if (role !== 'editor' && role !== 'owner') {
        redirect(303, `/login?redirect=${encodeURIComponent(url.pathname)}`);
    }
};
