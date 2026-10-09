//! What a new shell in a Space starts with.

use super::{
    AccountRef, Credential, DotenvFault, Profile, Project, RuntimeDir, agent_plan, dotenv_path,
    launch,
};
use crate::error::CoreError;
use crate::vocab::text::{EnvKey, EnvValue, KeyPath, ProjectRoot, RelPath};
use ds_style::space::list::SpaceId;

fn var(key: &str, value: &str) -> (EnvKey, EnvValue) {
    (EnvKey::new(key).unwrap(), EnvValue(value.to_owned()))
}

fn project() -> Project {
    Project {
        root: Some(ProjectRoot("/home/me/ops/".into())),
        profile: Profile {
            env: vec![var("REGION", "eu"), var("EDITOR", "hx")],
            ssh_identity: Some(KeyPath("~/.ssh/ops".into())),
            dotenv: Some(RelPath(".env".into())),
            credentials: vec![Credential {
                env: EnvKey::new("TOKEN").unwrap(),
                account: AccountRef::new("ops-token").unwrap(),
            }],
        },
        routes: vec![],
    }
}

#[test]
fn the_environment_layers_dotenv_then_profile_then_the_agent() {
    let runtime = RuntimeDir("/run/user/1000/".into());
    let dotenv = "REGION=us\nDB=pg\nSSH_AUTH_SOCK=/elsewhere\nbad line\n";
    let start = launch(SpaceId(2), &project(), Some(dotenv), &runtime);
    assert_eq!(
        start.env,
        vec![
            var("REGION", "eu"),
            var("DB", "pg"),
            var("SSH_AUTH_SOCK", "/run/user/1000/temor/agent-2.sock"),
            var("EDITOR", "hx"),
        ],
        "the profile beats the file, the agent beats both, a name keeps its first place"
    );
    assert_eq!(start.dotenv_faults, vec![(4, DotenvFault::NoEquals)]);
    assert_eq!(start.cwd.map(|c| c.0), Some("/home/me/ops/".to_owned()));
    assert_eq!(start.credentials, project().profile.credentials);
    let agent = start.agent.expect("the profile holds a key");
    assert_eq!(agent.identity, KeyPath("~/.ssh/ops".into()));
}

#[test]
fn a_bare_project_starts_a_plain_shell() {
    let start = launch(
        SpaceId(0),
        &Project::default(),
        None,
        &RuntimeDir("/run/user/1000".into()),
    );
    assert_eq!(start, super::SpaceLaunch::default());
}

#[test]
fn the_agent_socket_must_fit_in_a_socket_address() {
    let profile = project().profile;
    let long = RuntimeDir(format!("/run/{}", "x".repeat(100)));
    assert!(matches!(
        agent_plan(SpaceId(1), &profile, &long),
        Err(CoreError::SocketPathTooLong { max: 107, .. })
    ));
    let start = launch(SpaceId(1), &project(), None, &long);
    assert_eq!(start.agent, None);
    assert!(matches!(
        start.agent_fault,
        Some(CoreError::SocketPathTooLong { .. })
    ));
    assert_eq!(
        agent_plan(SpaceId(1), &Profile::default(), &long),
        Ok(None),
        "no key, no agent, so no socket to check"
    );
}

#[test]
fn the_dotenv_file_stays_inside_the_project() {
    let with = |rel: &str, root: Option<&str>| Project {
        root: root.map(|r| ProjectRoot(r.to_owned())),
        profile: Profile {
            dotenv: Some(RelPath(rel.to_owned())),
            ..Profile::default()
        },
        routes: vec![],
    };
    assert_eq!(
        dotenv_path(&with("config/.env", Some("/p/"))),
        Ok(Some("/p/config/.env".to_owned()))
    );
    assert_eq!(dotenv_path(&with(".env", None)), Ok(None));
    assert_eq!(dotenv_path(&Project::default()), Ok(None));
    for bad in ["/etc/passwd", "../.env", "a/../../b", ""] {
        assert_eq!(
            dotenv_path(&with(bad, Some("/p"))),
            Err(CoreError::BadDotenvPath(bad.to_owned())),
            "{bad:?}"
        );
    }
}

#[test]
fn account_references_are_slugs() {
    for good in ["a", "ops-token", "x.y_z-9", &"a".repeat(64)] {
        assert!(AccountRef::new(good).is_ok(), "{good}");
    }
    for bad in ["", "Ops", "-a", "a b", "a/b", &"a".repeat(65)] {
        assert!(AccountRef::new(bad).is_err(), "{bad}");
    }
}
