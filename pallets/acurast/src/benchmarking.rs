use acurast_common::{AttestationChain, JobRegistration, Script};
use frame_benchmarking::{account, benchmarks, whitelist_account};
use frame_support::{
    assert_ok,
    sp_runtime::traits::{AccountIdConversion, Get, StaticLookup},
    traits::OriginTrait,
};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

pub use pallet::Config;

use crate::utils::validate_and_extract_attestation;
use crate::Pallet as Acurast;

use super::*;

pub type Balance = u128;

pub const SEED: u32 = 1337;
pub const ROOT_CERT: [u8; 1312] = hex!("3082051c30820304a003020102020900d50ff25ba3f2d6b3300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139313132323230333735385a170d3334313131383230333735385a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a3633061301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201004e31a05cf28ba65dbdafa1ced70969ee5ca84104added8a306cf7f6dee50375d745ed992cb0242cce72dc9eed51191fe5ad52bad7dd3b25c099e13a491a3cdd487a5acce8766324c4ae46338246ae7b78a418acbb98a05c4c9d696eeaab609d0ba0ce1a31be98490df3f4c0ea9ddc9e82ffb0fcb3e9ebdd8cb952789f2b1411fac56c886426eb7296042735da50e11ac715f1818cf9fdc4e254a3763351b6a2440150861263a6e310be1a50de5c7e8ee880fdd4be5884a37128d18830bb3476bf4291e82d5c66a6494939e08480bfbc00f7d8a74d43e73737ebe5d8e4ec515302d4689692780dc7538ed7e9175be6139e74d43ad388b3050ffd5a9de5262000898c01f63c53dfe22209108fa4f65ba16c49ccbde0837d7c5844d54b7398ba0122e505b155c9313cfe26e72d87e22aa1616e6bdbf547ddff93df29e35a63b455fe1fc0ec95581f3f4f7bbe3bb828396a37ae3157582bc3764b9780a239efc0f75a1e2e6d941ceabac27ddeb01e2bd8421029bea34d51aee6c60271d5a95ebd00515a9c0013dd80bf87eea260b81c34f688e6eb1348af0d8ea1cac32acb9d93fa24aff030a84c8f2b0f569cc95080b20ac35ace0c6d8dbd4f6847719519d32450166eb4bf15b859044501adeaf436382c34b15e3b54c92e61b69c2bfc7264589172b3c93dbe35ce06d08fd5c01322ca0877b1d12743af1fad5940ea1bc02dd891c");
pub const INT_CERT_1: [u8; 920] = hex!("308203943082017ca003020102021100d6611e75cba6538cf98a8af9c548a9e6300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3231303131333230353434395a170d3331303131313230353434395a3039310c300a060355040c0c03544545312930270603550405132033303063666163306166633735656233353935396166303934656338376539333076301006072a8648ce3d020106052b8104002203620004ffed6b48eb73ec1e1558ad7e0d8906a8e2438a659fd217bc477bce4a5e6a5917510de4db7e4191109215be36d3e3bf03e3c791afd52e2df367cd0b1d1134e4c477384c4792f6de9c333b0d8529ddbbb0e9ec639cde0c90175be76286c8bf3c1ea3633061301d0603551d0e0416041485e7bd7db7ba5948e99002ee53fd2621b1a611a3301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b05000382020100289461ab377d68173e1201d952c606987517a031bc9c8b207b9ae4ae50653ebf5f00e4d7e23f0ba9b59d270ac4379f72ac6829e7cf658a609439a6e137e5791bd806d775bce6f30bbab15a647a736a506c08c5a3cb0b3139a23914f9057045fac6dc6b68bc105056896eeabe164f9fb04e1c8fd8a7e743411c76c6a82241cbee199021bd3c3ea8046b3c057a50d538fafad64c27e7454cd423b1fc1423c2f861e51fec4a8cdc579135b8cfcb23a404e26f5bf55ad809ad89986b1059b12ea2ea587afb1909e92e624a83fe6608850afd08ccda6a525daba425f54f772a28dc5868a907827e7c8885cfa77b3d3ce15969e000f70da69364c63d6d38996697032d1b46753846db768f251163aa37c20a23c942b569bda145928bf967ebf64e4bea437e6f82e1590b5c6f12e1f4fbe0f9a08e5dab76bf546ab2cec2731203e25587d52b2eec09db81f32ac7242e20eaf55040140d43718554f952955b977fac67e469864a119eb25d28c7ac7b9c650000d6a90010c568a0d9c45a89c76bad232bcf9b6de72fb782a453446d3d5cf959c68458ab962a205701c5452b8b03ad79669760872c0623e84fbf449fdc8e48efacf2d312dee6839c0c70c72c45168810cfc57e656513aea347fd9254dbf7958eb8e6e722034851e472f16808ab37b6a452bda644d46a2356635d22690af0c08908f59896fd455efb5984a1da807c2670de6b");
pub const INT_CERT_2: [u8; 505] = hex!("308201f53082017aa003020102021100dff1d9f42cf86174fad78c8ab75bb3e1300a06082a8648ce3d0403023039310c300a060355040c0c0354454531293027060355040513203330306366616330616663373565623335393539616630393465633837653933301e170d3231303131333230353733395a170d3331303131313230353733395a3039310c300a060355040c0c03544545312930270603550405132030646232373437633639623836343736666162303539346139336436323030333059301306072a8648ce3d020106082a8648ce3d030107034200049ed4960871506dce77605b590c0bd75a875e6b5847a11b670491b1f595398788709d97293ec3512a3ab6a2f11024e312ecacb7ec68fc9bb2e1a8223d5f9cea74a3633061301d0603551d0e0416041425c651f090c019477e803623ad18991898d12a8f301f0603551d2304183016801485e7bd7db7ba5948e99002ee53fd2621b1a611a3300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300a06082a8648ce3d0403020369003066023100bdbc6a2c566f5cbc747e3cc8c7de7931ab0c27c5d459ca89801791ce0badac455bad81022281915794de57e40105796d023100bbc9632c2eb0edb5476502b46c5d757627c6af0297db3ecef24bc31cca82faca888986ccef681756d29fe36966546f7b");
pub const LEAF_CERT: [u8; 663] = hex!("3082029330820239a003020102020101300a06082a8648ce3d0403023039310c300a060355040c0c03544545312930270603550405132030646232373437633639623836343736666162303539346139336436323030333020170d3730303130313030303030305a180f32313036303230373036323831355a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d030107034200047722895e4bb14fa898023204b6d5a7257db4ec6dcf35c5ee3b9cf107185c3d7e3102320f361830eba030aa0350fad89aa89d126c07769a9c638ddc639819d838a382014830820144300e0603551d0f0101ff04040302078030820130060a2b06010401d679020111048201203082011c0201030a01010201040a0101040004003066bf853d08020601847d243620bf85455604543052312c302a0425636f6d2e616375726173742e61747465737465642e6578656375746f722e746573746e657402010e31220420241582199f0356954f490401727f2456092d27ba9b3987c18c79448a2d53fa853081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420c5d3c71bc70d58e3e0409ca9d9b34c0dbac1d2f09a5de948a4b8f090f19269650101ff0a010004207799155a5f44f15b94cf8817146ce62f270052b9ce6c864bc938b6a3dd6ed285bf854105020301adb0bf85420502030315d9bf854e060204013488c5bf854f060204013488c5300a06082a8648ce3d0403020348003045022100a0774a33cafa6a7e397e4e66f75d3adf83e4535d119afbfba1be4cff91ed894b022010c0c4cb9d2cfa468121558b2cbae27a3432dc83b3716817e63841db470a3e07");
pub const INITIAL_BALANCE: u128 = UNIT * 10;
pub const UNIT: Balance = 1_000_000;
const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
pub fn script() -> Script {
    SCRIPT_BYTES.to_vec().try_into().unwrap()
}
pub fn attestation_chain() -> AttestationChain {
    AttestationChain {
        certificate_chain: vec![
            ROOT_CERT.to_vec().try_into().unwrap(),
            INT_CERT_1.to_vec().try_into().unwrap(),
            INT_CERT_2.to_vec().try_into().unwrap(),
            LEAF_CERT.to_vec().try_into().unwrap(),
        ]
        .try_into()
        .unwrap(),
    }
}
pub fn processor_account<T: Config>() -> T::AccountId
where
    T::AccountId: From<[u8; 32]>,
{
    let pub_key = hex!("9766feff8a676838cc9d2bd20e977db9920ac25d136d93453e0b2d2571fb7789");
    // codec::Decode::decode(&mut sp_runtime::traits::TrailingZeroInput::new(
    //     pub_key.as_ref(),
    // ))
    // .expect("infinite length input; no invalid inputs for type; qed")
    pub_key.into()
}

pub fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn job_registration<T: Config>() -> JobRegistrationFor<T> {
    return JobRegistration {
        script: script(),
        allowed_sources: None,
        allow_only_verified_sources: false,
        extra: T::RegistrationExtra::benchmark_default(),
    };
}

///

pub fn consumer_account<T: Config>() -> T::AccountId {
    account("consumer", 0, SEED)
}

fn register_job<T: Config>(submit: bool) -> (T::AccountId, JobRegistrationFor<T>) {
    let consumer: T::AccountId = consumer_account::<T>();
    whitelist_account!(consumer);

    let job = job_registration::<T>();

    if submit {
        let register_call =
            Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone());
        assert_ok!(register_call);
    }

    (consumer, job)
}

fn assign_job<T: Config>(
    submit: bool,
    job: JobRegistrationFor<T>,
) -> JobAssignmentUpdate<T::AccountId>
where
    T::AccountId: From<[u8; 32]>,
{
    let consumer_account: T::AccountId = consumer_account::<T>();
    let processor_account: T::AccountId = processor_account::<T>();

    let job_assignment = JobAssignmentUpdate {
        operation: ListUpdateOperation::Add,
        assignee: processor_account,
        job_id: (consumer_account.clone(), job.script.clone()),
    };

    if submit {
        let assignment_call = Acurast::<T>::update_job_assignments(
            RawOrigin::Signed(consumer_account).into(),
            vec![job_assignment.clone()],
        );
        assert_ok!(assignment_call);
    }

    job_assignment
}

benchmarks! {
    where_clause {  where
        T: pallet_timestamp::Config,
        <T as pallet_timestamp::Config>::Moment: From<u64>,
        T::AccountId: From<[u8; 32]>,
    }

    register {
        let (caller, job) = register_job::<T>(false);
    }: _(RawOrigin::Signed(caller.clone()), job.clone())
    verify {
        assert_last_event::<T>(Event::<T>::JobRegistrationStored(
            job, caller
        ).into());
    }

    deregister {
        let (caller, job) = register_job::<T>(true);
    }: _(RawOrigin::Signed(caller.clone()), job.script.clone())
    verify {
        assert_last_event::<T>(Event::<T>::JobRegistrationRemoved(
            job.script, caller
        ).into());
    }

    update_allowed_sources {
        let (caller, job) = register_job::<T>(true);
        let sources_update = vec![AllowedSourcesUpdate {
            operation: ListUpdateOperation::Add,
            account_id: processor_account::<T>(),
        }];

    }: _(RawOrigin::Signed(caller.clone()), job.script.clone(), sources_update.clone())
    verify {
        assert_last_event::<T>(Event::AllowedSourcesUpdated(
            caller, job, sources_update
        ).into());
    }

    // requires xcm messaging to succeed, which is not possible when running the benchmark.
    // this extrinsic will change very soon so no point in trying to make it succeed for now
    // fulfill {
    //     let (caller, job) = register_job::<T>(true);
    //     let job_assignment = assign_job::<T>(true, job.clone());
    //     let fulfillment = Fulfillment {
    //         script: job.script.clone(),
    //         payload: hex!("00").to_vec(),
    //     };
    //
    // }: _(RawOrigin::Signed(job_assignment.assignee.clone()), fulfillment.clone(), T::Lookup::unlookup(caller.clone()))
    // verify {
    //     assert_last_event::<T>(Event::ReceivedFulfillment(
    //         job_assignment.assignee.clone(),
    //         fulfillment,
    //         job,
    //         caller
    //     ).into());
    // }

    submit_attestation {
        let processor_account = processor_account::<T>();
        let attestation_chain = attestation_chain();
        // notAfter: 4,294,967,295,000
        let timestamp_call = pallet_timestamp::Pallet::<T>::set(T::Origin::none(), 2_000_000_000_000_u64.into());
        assert_ok!(timestamp_call);

    }: _(RawOrigin::Signed(processor_account.clone()), attestation_chain.clone())
    verify {
        let attestation = validate_and_extract_attestation::<T>(&processor_account, &attestation_chain).unwrap();
        assert_last_event::<T>(Event::AttestationStored(
            attestation,
            processor_account,
        ).into());
    }

    update_certificate_revocation_list {
        let updates =  vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            cert_serial_number: hex!("15905857467176635834").to_vec().try_into().unwrap()
        }];

        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

    }: _(RawOrigin::Signed(pallet_account.clone()), updates.clone())
    verify {
        assert_last_event::<T>(Event::CertificateRecovationListUpdated(
            pallet_account,
            updates
        ).into());
    }
    // impl_benchmark_test_suite!(Acurast, mock::ExtBuilder::default().build(), mock::Test);
    impl_benchmark_test_suite!(Acurast, mock::ExtBuilder::default().build(), mock::Test);
}
