# Test the event stream connection to a NATS server

Describe "event stream not connected to nats" {
    $env:RUST_LOG = "rants=trace"

    It "fails to start with --event-stream-connect-timeout set" {
        {
            $supLog = New-SupervisorLogFile("event_stream-fails_to_start_with_no_NATS_server_with_timeout")
            Start-Supervisor -LogFile $supLog -Timeout 3 -SupArgs @( `
                    "--event-stream-application=MY_APP", `
                    "--event-stream-environment=MY_ENV", `
                    "--event-stream-site=MY_SITE", `
                    "--event-stream-url='127.0.0.1:4222'", `
                    "--event-stream-token=blah", `
                    "--event-stream-connect-timeout=2" `
            )
        } | Should -Throw
    }
}

Describe "event stream connected to automate" {
    BeforeAll {
        Write-Host "Building automate image..."
        docker build -t automate ./test/end-to-end/automate
        Write-Host "starting automate container..."
        $script:cid = docker run --rm -d -p 4222:4222 automate
        Write-Host "Waiting for automate to get healthy..."
        docker exec $cid chef-automate status -w
        Write-Host "Automate is healthy!"
        $authToken = $(docker exec $cid chef-automate iam token create my_token --admin)
        Write-Host "Obtained token: $authToken"
        $cert = New-TemporaryFile
        docker exec $cid chef-automate external-cert show | Out-File $cert -Encoding utf8
        Write-Host "Retrieved server certificate to $cert"

        # Start the supervisor but do not require an initial event stream connection
        $supLog =  New-SupervisorLogFile("test_event_stream")
        Write-Host "Starting Supervisor..."
        Start-Supervisor -Timeout 45 -LogFile $supLog -SupArgs @( `
                "--event-stream-application=MY_APP", `
                "--event-stream-environment=MY_ENV", `
                "--event-stream-site=MY_SITE", `
                "--event-stream-url=localhost:4222", `
                "--event-stream-token=$authToken", `
                "--event-stream-server-certificate=$cert" `
        )
        Write-Host "Loading test-probe"
        Load-SupervisorService -PackageName "habitat-testing/test-probe"
        Write-Host "Service Loaded"
    }

    AfterAll {
        Unload-SupervisorService -PackageName "habitat-testing/test-probe" -Timeout 20
        Stop-Supervisor
        docker stop $script:cid
        docker rmi -f automate
    }

    It "connects and sends a health check" {
        # test-probe has a long init hook, and we want
        # to let the health-check hoo
        Start-Sleep -Seconds 20

        # Check that the output contains a connect message and that the server received a health check message
        $out = (docker exec $cid chef-automate applications show-svcs --service-name test-probe) | Out-String
        $out.Trim() | Should -BeLike "*OK"
    }
}
