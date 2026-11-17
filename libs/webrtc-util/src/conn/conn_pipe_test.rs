use super::conn_pipe::*;
use super::*;

#[tokio::test]
async fn test_pipe() -> Result<()> {
    let (c1, c2) = pipe();
    let mut b1 = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let n = c1.send(&b1).await?;
    assert_eq!(n, 10);

    let mut b2 = vec![133; 100];
    let n = c2.recv(&mut b2).await?;
    assert_eq!(n, 10);
    assert_eq!(&b2[..n], &b1[..]);

    let n = c2.send(&b2[..10]).await?;
    assert_eq!(n, 10);
    let n = c2.send(&b2[..5]).await?;
    assert_eq!(n, 5);

    let n = c1.recv(&mut b1).await?;
    assert_eq!(n, 10);
    let n = c1.recv(&mut b1).await?;
    assert_eq!(n, 5);

    Ok(())
}
