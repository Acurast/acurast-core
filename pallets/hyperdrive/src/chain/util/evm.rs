use crate::MessageIdentifier;
use rlp::{decode_list, encode, RlpStream};
use sp_core::Hasher;
use sp_runtime::traits::Keccak256;
use sp_std::vec::Vec;

// Reference implementation taken from: https://github.com/a16z/helios/blob/cb50725acc8c6c28e55cb91cec9bb415d2c97ea5/execution/src/proof.rs

pub fn verify_proof(proof: &Vec<&[u8]>, root: &[u8], path: &Vec<u8>, value: &Vec<u8>) -> bool {
    let mut expected_hash = root.to_vec();
    let mut path_offset = 0;

    for (i, node) in proof.iter().enumerate() {
        if expected_hash != Keccak256::hash(node).0 {
            return false;
        }

        let node_list: Vec<Vec<u8>> = decode_list(node);

        if node_list.len() == 17 {
            if i == proof.len() - 1 {
                // exclusion proof
                let nibble = get_nibble(path, path_offset);
                let node = &node_list[nibble as usize];

                if node.is_empty() && is_empty_value(value) {
                    return true;
                }
            } else {
                let nibble = get_nibble(path, path_offset);

                expected_hash = node_list[nibble as usize].clone();

                path_offset += 1;
            }
        } else if node_list.len() == 2 {
            if i == proof.len() - 1 {
                // exclusion proof
                if !paths_match(&node_list[0], skip_length(&node_list[0]), path, path_offset)
                    && is_empty_value(value)
                {
                    return true;
                }

                // inclusion proof
                if &node_list[1] == value {
                    return paths_match(
                        &node_list[0],
                        skip_length(&node_list[0]),
                        path,
                        path_offset,
                    );
                }
            } else {
                let node_path = &node_list[0];
                let prefix_length = shared_prefix_length(path, path_offset, node_path);
                if prefix_length < node_path.len() * 2 - skip_length(node_path) {
                    // The proof shows a divergent path, but we're not
                    // at the end of the proof, so something's wrong.
                    return false;
                }
                path_offset += prefix_length;
                expected_hash = node_list[1].clone();
            }
        } else {
            return false;
        }
    }

    false
}

fn paths_match(p1: &Vec<u8>, s1: usize, p2: &Vec<u8>, s2: usize) -> bool {
    let len1 = p1.len() * 2 - s1;
    let len2 = p2.len() * 2 - s2;

    if len1 != len2 {
        return false;
    }

    for offset in 0..len1 {
        let n1 = get_nibble(p1, s1 + offset);
        let n2 = get_nibble(p2, s2 + offset);

        if n1 != n2 {
            return false;
        }
    }

    true
}

fn is_empty_value(value: &Vec<u8>) -> bool {
    let mut stream = RlpStream::new();
    stream.begin_list(4);
    stream.append_empty_data();
    stream.append_empty_data();
    let empty_storage_hash = "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421";
    stream.append(&hex::decode(empty_storage_hash).unwrap());
    let empty_code_hash = "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
    stream.append(&hex::decode(empty_code_hash).unwrap());
    let empty_account = stream.out();

    let is_empty_slot = value.len() == 1 && value[0] == 0x80;
    let is_empty_account = value == &empty_account;
    is_empty_slot || is_empty_account
}

fn shared_prefix_length(path: &Vec<u8>, path_offset: usize, node_path: &Vec<u8>) -> usize {
    let skip_length = skip_length(node_path);

    let len = core::cmp::min(
        node_path.len() * 2 - skip_length,
        path.len() * 2 - path_offset,
    );
    let mut prefix_len = 0;

    for i in 0..len {
        let path_nibble = get_nibble(path, i + path_offset);
        let node_path_nibble = get_nibble(node_path, i + skip_length);

        if path_nibble == node_path_nibble {
            prefix_len += 1;
        } else {
            break;
        }
    }

    prefix_len
}

fn skip_length(node: &Vec<u8>) -> usize {
    if node.is_empty() {
        return 0;
    }

    let nibble = get_nibble(node, 0);
    match nibble {
        0 => 2,
        1 => 1,
        2 => 2,
        3 => 1,
        _ => 0,
    }
}

fn get_nibble(path: &[u8], offset: usize) -> u8 {
    let byte = path[offset / 2];
    if offset % 2 == 0 {
        byte >> 4
    } else {
        byte & 0xF
    }
}

/// Obtain the storage path for the proof
pub fn storage_path(storage_index: &u8, message_id: &MessageIdentifier) -> [u8; 32] {
    let mut key_bytes: [u8; 32] = [0u8; 32];
    let message_id_encoded = encode(message_id);
    key_bytes[32 - message_id_encoded.as_ref().len()..]
        .copy_from_slice(message_id_encoded.as_ref());

    let mut storage_index_bytes: [u8; 32] = [0u8; 32];
    storage_index_bytes[31] = encode(storage_index)[0];

    let combined = [key_bytes, storage_index_bytes].concat();

    Keccak256::hash(&Keccak256::hash(combined.as_slice()).0).0
}

#[cfg(test)]
mod tests {
    use super::{shared_prefix_length, storage_path, verify_proof};
    use hex_literal::hex;

    #[test]
    fn test_calculate_proof_path() {
        let path = storage_path(&6, &1);
        assert_eq!(
            path.as_slice(),
            hex!("80497882cf9008f7f796a89e5514a7b55bd96eab88ecb66aee4fb0a6fd34811c").as_slice()
        );

        let path = storage_path(&4, &1);
        assert_eq!(
            path.as_slice(),
            hex!("210afe6ebef982fa193bb4e17f9f236cdf09af7788627b5d54d9e3e4b100021b").as_slice()
        );
    }

    #[test]
    fn test_shared_prefix_length() {
        // We compare the path starting from the 6th nibble i.e. the 6 in 0x6f
        let path: Vec<u8> = vec![0x12, 0x13, 0x14, 0x6f, 0x6c, 0x64, 0x21];
        let path_offset = 6;
        // Our node path matches only the first 5 nibbles of the path
        let node_path: Vec<u8> = vec![0x6f, 0x6c, 0x63, 0x21];
        let shared_len = shared_prefix_length(&path, path_offset, &node_path);
        assert_eq!(shared_len, 5);

        // Now we compare the path starting from the 5th nibble i.e. the 4 in 0x14
        let path: Vec<u8> = vec![0x12, 0x13, 0x14, 0x6f, 0x6c, 0x64, 0x21];
        let path_offset = 5;
        // Our node path matches only the first 7 nibbles of the path
        // Note the first nibble is 1, so we skip 1 nibble
        let node_path: Vec<u8> = vec![0x14, 0x6f, 0x6c, 0x64, 0x11];
        let shared_len = shared_prefix_length(&path, path_offset, &node_path);
        assert_eq!(shared_len, 7);
    }

    #[test]
    fn test_verify_account_state_proof() {
        let proof: Vec<&[u8]> = vec![
            &hex!("f90211a0f95b30e8057169e0fc1daa9f78787333a372e485d8e1d2c2d6e6490c3bd6016fa0681665dc7c7d2a1b6209c6f317c718eee11ab8eedd61fccf86c067a6e3806d27a07207538d7bfeebf3470e06fcdaee54257f5916b58116b26db1cf7b76f1159cb2a01686a528f93001316f899c817524945b9c9be4315dc338bfafa618c813a4e207a06c36fd3689e73ce2827b5f92d67a770eff3b50aa1347583e1322dbb368f23632a0f304a9873278d4a7883cbb1279f22ab463aff78049baa716c6afc1539f6597b8a01d5ebc8150378a4038a4abaaa3462fbbe73b9cd84e640d0c3e882b49a1ddafc2a0cb252bf4d64b84ed05e71d33394a21d8eb79c17efccdf4fc22a7616d7938b936a06e66ee831d3d94c099ae66dbe115b4ddbb6d25f74e80d5c518794b6780ce9384a01474af95a02eff151cccab14ddb3a742696cb4111c3ee6c9022e0320f67b3377a0fabd16e8b32fa448ef500b790994f20814926337176a15844b89477173fb807ba02e07808444c4f433715574baa5cee086b63e702921b14a201a2180db17d9cb4ca0ce71a37dc14cf4c103d685246b354c95662564142a5a150c070b36730c3d2634a07417c43675ea5cd1b7826d11d2dd9ddad001c5977d2928a2a00caa69a44509f0a0bce5abe6ef48bf0cd83c1b1dad37e6616b5acee93e055ef055698946a99edb58a060cd523662c2656342ca06a1914489c2d66b823a1996cabfa11888f8c5126f3580"),
            &hex!("f90211a04dad2e8c56b4d41a8bf62784c999d62946787aa89608e74e63e70db454a941dea06182484ec7e0ff2a22680b567bd979a0ed0883729192425be22346f66dbff7eaa09555cd7bd1a1f2f046e84af6293a1d90d427d1aa1e8532aa4d123d5b8a33872ba027d4b7804eeb1516fba785caebfe9cb13697b95d5b23b74119e0635b4f7aa3a9a02b18f67a754a345e573ae03aae01d3e371465d757999f0c41ef13eecd30a11cda096560ee6b086fb8e10d65d0ae75a6d8b093e226b06e181afd5f7869ec0327117a07954049a9a8256f41d22164429692c1cc75f1c6b604a088c79c5dc5778f05efaa007ffb113f7370423f31b3c0bb9e2c2e3513e0f48a8550fb42694b8a632b05c40a02291acda3ef7748c9aa6139832f0cc8c10e4227643d194ef11d4163b4cd36e01a03c71b4760b879e666c704c744036eb3ff585d085fe7fd82a08634140d6c98207a0daaed6465d195816bd1919fdd19688a6e2a9156ef351d106e2f1a07781cc9d57a058f9047b134ad2ecc5af428d272f5acc8d386249e1bef5bd6f96c18f457063cea032b4f66d9fc622bbab0c862a4b51aed5956b48ca9f05beb7da6d37b35a3263dda0dfefe8a210051438b50dd2f092d03300311d93e235aaeadcaf3e5681988c1feca0eeb5cbe3746d80b37387802ae393c36511e66b5080b3c767f0731588037e508aa03dcb5f831c18d2c6c6ca69e25a13266075960d81a632385ee3cb87d7a1e9843280"),
            &hex!("f90211a0d0fa35677e37b205b596cf4c212b479326befc3a4a9e18c6bfaa7c59643b9fd0a072fe41c97253aa0ec8dcac18ba5fd453f0eefdf850d10a39cd30524d20439452a0723752b0350d1ecefe1eba876307099c28fdac16f6e70a667a69d7d93fbf75cfa076e5549004b7168bc37c15e83b0c48966408c294c84c352c5fb650647b799292a00110a0b7592311b22cd1bc621e896861e1414546672baf6ecac1fdc618acc017a07c44a8353e9e0aad2cb10d6c6ea99b1546370138078b42ef37532314c705791ba0c5f785f12278ef0012b8aee57beefcd83ffd262c084c5eca786c06b916b01e85a001f1ef52f4a5c6a94a50694b595cc008a61f5571bfdf7d2b937bf1353c9acba7a039fee3000c0ac6daa58d716d8d412efcdc9bc0bd7939f7b7ca3b5a6cf0c061e0a0162e73de59897db33bd41bc8e09acdadb830df30e6036d5e7329f75285d3d914a02a83b09982f2d3f0a8b3730a768fbf2db12bfdc25dd383d5e8f4e5b7336b39ffa0bd052c8dcfe57c7ee8fe2ad4167ea361972cf68ce9dda5ffb14374ea72d1ac79a007b92b3f0c3cd17275ee20fae41512d59d04b5d3eef5cad3f48c0b21c0e17703a039abe83169b68fdf16d227d94a082cf112cfa9085208fe6b5896cd159d0588aba08261e532414b5bcd0929a7a950ea165e7e3ff54af23b3b460b9e862e22770260a01eb72340083708752d3aa06f2c0a3a1047e611aa1a07b7ed676d95401e5afe7480"),
            &hex!("f90211a0f3ae0e4692920a9093eba3b2810597ee80831ff6864fb526e0def51b538dc6eca09a33c672b1119c28d097b1f96afc69ad75f2bb175122761497d79a2c92f107f1a0c9936539b9bbad200f27dd0589c937e98d7cb627fb67add1f096eb50d75a3bada022e2f4fb91f2961dfc16e3e30da98ffe800ea24d094f383ca6bc1b66cf70d2d2a03a25c8cbbc6e2a236aff16e2415051f64642e04bdc4079ae38fc72def1c8be06a067639ea05f1846205a827ffc135ce949fede0072dcfa721b8d167ce61ba6898ba060c8df7225b4e06efb17e6e09568bcbc43bc0156fbd498abb99ed77adc4ae8f7a03999adfb9a4c463c56e3648b6853cf2a24cdeef40ddb278c77b868471e8c51a8a09be11017032234ac90a19821f492541824f024b4fdc00f031d7267193837f998a06568372ff5d4d33b6012026b85ac82317d7d941708ce3730ac3c148e5ca92c1aa055cd80c4a73ac9a4f87aaf626b891c1aa8e72699a1f8ab1f6f1389df679d6ebba01bf003aa86731cc1a0fff8896794ddf52efbfee182f894ecf6fadc17a79286f4a0f07f8e6d8a6c8926af30e22ef20fdf4d50a3cbf1699dff7494d8b5cccc5bee91a0b346e0b13d93f37be6498e91a7b09944f52a372d21020310775063e1398cabbea0d8fb399de1d5e3d9eb7c37e171771e4018ee0f4d69e7c879ebc3ad9850802d3fa07a18971c78172d4ffa80117d5318340f69dbb651c96baa205b6aed810740ca7c80"),
            &hex!("f90211a0855b55c2ab89eb13f31dbf1b713b3fbcbd44319a6e82f5b88a5a81fff37be2dca00242150c331fd426bab884e917f543987d8691f2a80bfd0506c5f226dc06bc27a009aed285d92badd8eeef5ea976ea37b37aa377acebc37dd8b72e9056d98638dda0497aad06d0e1536194a50eef2b5204a8ce2115dac71d1162d0638d183505b6fda0bcf9e7972ba2006afc8f6d75754d3144605d2568e627a8f8dcc657f87d6c4179a061fc222341591278641e4f7f23950699b5d6e556b55578628995db53ff9074f1a0f2dd11b267eda5dc67430e1ac88e3e868d02d2507d5146a40471122030441e6aa02488c6d04fab1938154c329fbada80fcdafa301ba28787e3929603ad49122e2ea0db97dfaeb2f81f4e62f24377e48b99a9c0c40add9e13b2f6acc6e6c7b9a05e61a0fe3344535fbf172577e0aa65b95bd831340e798153bbd372408c482e9104d5bea02f4a722816303116ab4562c5334bcbff7b3d59a1d6a64292ec59796291078df5a02c6313c5d25d45e203e5b836f3e4646d25f929c2b0899e44521452b35d80ef20a0d6a2361bfc61aba1ce27408d898c60c25212015547c4cda1ec354ba204e369f9a095de06eff0d25c8f783ac5dd6cdfd19b5076612d43181f27be0ea3b725668ec5a0f2873eb65424aaecb09658cbf2aa355dea12fda2f77d39ad5a50b1f77a47cf08a08399367c73f9de5cf54e56f9049aaab18b3463f69d769f62ecf838d1ae967e4f80"),
            &hex!("f87180a09561a997c264962c9a8b4aee2582b8ef36a189e3e726d35c2cb826fe8d0fd87d80808080a00010dfaea0e22d6ff3ef10c217e1c415911b21a17c987a26f23c3a01e89b3d0a808080808080a0be6964353ef31b78edaaad0e8bd87b64d62632aef85fdd3ae963955a98628ec2808080"),
            &hex!("f8679e20389db67e3b84adf9d34deca5638f3aaf86a8eaaa6147889bca489e7a7cb846f8440180a0c043666e3ecc8c280ba165497aa3ed83dddba54c00e6e73486c68427925e0778a0e751f0a9365eab5149f29145082d5b033520eb9cb2432527d65e19d6efcbdd0b"),
        ];
        let path =
            hex!("14c9bd389db67e3b84adf9d34deca5638f3aaf86a8eaaa6147889bca489e7a7c").to_vec();
        let value = hex!("f8440180a0c043666e3ecc8c280ba165497aa3ed83dddba54c00e6e73486c68427925e0778a0e751f0a9365eab5149f29145082d5b033520eb9cb2432527d65e19d6efcbdd0b").to_vec();
        let root = hex!("297677d612641f8a53454bc8126f4b225b95ddb6ab395d12a2ed740b8ca81cd4");
        assert!(verify_proof(&proof, &root, &path, &value));
    }

    #[test]
    fn test_verify_storage_proof() {
        let proof: Vec<&[u8]> = vec![
            &hex!("f90111a04c77a8959da29908fa97ea8718d3dd2fc298c353a9da9e09c6131a6a1cc3de8880a0308611a8afda5c8a10b09de3fed011ae43c480313fd2c85d65a92d35359de7fb808080a0f088bca3be2219e02d2ce722d00fdf516680747991013835a1c30d5296b47fec80a0017e20495a1d135325ad9f1f72d720a0b20b85eca8319f10f8c6f461a62e27bf8080a0482e10e64fe37936565267fcb8e0dd9cf74303ab0ce750dd5437bfdd99249528a0b794f22030a6452bd30975ba1b9dee4b798f3b560807323473a401da7f87124980a01f0b30aa51df7ff59d462dcefc151653f1af532a650eb9a5c59672dcf751a5f7a02f948e17d693c90a394a6dff75aa79461702f6361a43daee7f3eaa143825489d80"),
            &hex!("f87180a0367682f42ce7bfb86a31cc6924f7038a750e822f31c0e905b51f5ddf9b8dfb2380808080808080a04d0c15612e60ae90c040ff5eef0f99778a6f3dfdbdfacf954295252cef782a108080808080a02e5c6b3fb31df33a8f3f8e62ddfb6ef3078682b4aa8e1748b4fde838aaac742e80"),
            &hex!("f843a0200afe6ebef982fa193bb4e17f9f236cdf09af7788627b5d54d9e3e4b100021ba1a05f786a9fcb8250a3f27ed9192c66594dec76f3d53a4bf9d27ffc086b5196280d")
        ];

        let path =
            hex!("210afe6ebef982fa193bb4e17f9f236cdf09af7788627b5d54d9e3e4b100021b").to_vec();
        let value =
            hex!("a05f786a9fcb8250a3f27ed9192c66594dec76f3d53a4bf9d27ffc086b5196280d").to_vec();
        let root = hex!("c043666e3ecc8c280ba165497aa3ed83dddba54c00e6e73486c68427925e0778");
        assert!(verify_proof(&proof, &root, &path, &value));
    }

    #[test]
    fn test_verify_storage_proof2() {
        let proof: Vec<&[u8]> = vec![
            &hex!("f90131a0fe1cec69138a035b27919cba7d03d2f3b5867e183fc5928af3bc0b0f85b562a880a0e759fad30e475a8a7de20efb084aeaad48864ef0c5eb678f0133226a4489d5f8a0da9cbdd2154724e704491b792e162e096df39e9f51363b9b950933a61186820280a0de572a50aef9d550512795e67eaf06acda25ada12d45e5944fba2cb429641f5480a05abb50d3ee32dffe73e3a7f9f354bffe92e4971bf45b527d046208a6818120f980a0ca5985306e251400a05df43a16a3391bca6cf1e5a39acfde6f619c8ea03e3fbc8080a03783bc2fd4d98095264ccacf2098c92a04e317f93a82d87a713d645e0743ef8a80a0bb7fbc81f9cb125fa6229c00a3b6442d31316510f0a9054827bd2317fc95ac9ba0b60d522b76ccaef75c1d5d2faf67a3904ea0aadfe459950661a60e2111e94ca680"),
            &hex!("f8518080808080808080a0ff1d82682091977c3bd249fd5840706e2c8f487add0b1ae09d430e80d9aeb8f9808080808080a066ba505307e91ddbb884cf21cfffd24941ca533e0b9384a68144039ab7fc57a280"),
            &hex!("f843a0202ead72d53401d823f4de3290714b95c588de2c574133f57728a2d3d3763d3aa1a0f03ee4236f341d60bc114bdc519db37d120d1d98b8d3f12b9b6a65c2aa99b01d")
        ];

        let path =
            hex!("ff2ead72d53401d823f4de3290714b95c588de2c574133f57728a2d3d3763d3a").to_vec();
        let value =
            hex!("a0f03ee4236f341d60bc114bdc519db37d120d1d98b8d3f12b9b6a65c2aa99b01d").to_vec();
        let root = hex!("c53cbaddd072fc5094f0e0986a1baff9ed3d6dbe4133eb4e7764dd9e93f9ec9d");
        assert!(verify_proof(&proof, &root, &path, &value));
    }
}
