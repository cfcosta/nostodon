create type scheduled_post_status as enum ('new', 'running', 'errored', 'finished');

create table scheduled_posts (
  id serial primary key,
  user_id uuid not null,
  instance_id uuid not null,
  mastodon_id text not null,
  content text not null,
  profile_name text not null,
  profile_display_name text not null,
  profile_about text not null,
  profile_picture text not null,
  profile_nip05 text not null,
  profile_banner text not null,
  status scheduled_post_status not null default 'new',
  fail_reason text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create unique index scheduled_posts_mastodon_id_unique_idx on scheduled_posts (mastodon_id);
create trigger fill_scheduled_posts_updated_at_on_update before update on users for each row execute procedure fill_updated_at_on_update();

create or replace function scheduled_posts_status_notify()
	returns trigger as
$$
begin
	perform pg_notify('scheduled_posts_status_channel', new.id::text);
	return new;
end;
$$ language plpgsql;

create trigger scheduled_posts_status
	after insert or update of status
	on scheduled_posts
	for each row
execute procedure scheduled_posts_status_notify();
